use std::{
    collections::HashSet,
    env,
    sync::mpsc,
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    aws_ssm::{
        create_parameter_in_ssm, load_all_parameters_from_ssm, load_parameter_names_from_ssm,
        start_value_worker_pool,
    },
    models::{CreateField, FullRefreshResult, Parameter, ParameterMeta, ValueEditorMode, ValueWorkerPool},
};

pub struct App {
    pub selected: usize,
    pub all_parameters: Vec<Parameter>,
    pub filtered_indices: Vec<usize>,
    pub search_mode: bool,
    pub query: String,
    pub status: String,
    pub aws_region: String,
    pub create_mode: bool,
    pub create_name: String,
    pub create_value: String,
    pub create_field: CreateField,
    pub create_name_cursor: usize,
    pub create_value_cursor: usize,
    pub create_value_mode: ValueEditorMode,
    pub value_pool: Option<ValueWorkerPool>,
    pub pending_value_requests: HashSet<String>,
    pub full_refresh_rx: Option<mpsc::Receiver<Result<FullRefreshResult, String>>>,
}

impl App {
    pub fn new() -> Self {
        let region_from_env = env::var("AWS_REGION")
            .ok()
            .or_else(|| env::var("AWS_DEFAULT_REGION").ok())
            .unwrap_or_default();
        let configured_region = if region_from_env.trim().is_empty() {
            "default-chain".to_string()
        } else {
            region_from_env
        };
        let region_opt = if configured_region == "default-chain" {
            None
        } else {
            Some(configured_region.clone())
        };

        let (all_parameters, status, value_pool) = match load_parameter_names_from_ssm(region_opt.clone()) {
            Ok((params, count, thread_count)) => match start_value_worker_pool(thread_count, region_opt.clone()) {
                Ok(pool) => (
                    params,
                    format!(
                        "Loaded {count} parameter names from region {}. Lazy value loading with {thread_count} workers",
                        configured_region
                    ),
                    Some(pool),
                ),
                Err(err) => (
                    params,
                    format!(
                        "Loaded {count} names but lazy value workers failed: {err}. Values unavailable"
                    ),
                    None,
                ),
            },
            Err(err) => (Vec::new(), format!("SSM load failed ({err})."), None),
        };

        let mut app = Self {
            selected: 0,
            all_parameters,
            filtered_indices: Vec::new(),
            search_mode: false,
            query: String::new(),
            status,
            aws_region: configured_region,
            create_mode: false,
            create_name: String::new(),
            create_value: String::new(),
            create_field: CreateField::Name,
            create_name_cursor: 0,
            create_value_cursor: 0,
            create_value_mode: ValueEditorMode::Insert,
            value_pool,
            pending_value_requests: HashSet::new(),
            full_refresh_rx: None,
        };
        app.apply_filter();
        app
    }

    pub fn next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.filtered_indices.len();
    }

    pub fn previous(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.filtered_indices.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub fn start_search(&mut self) {
        self.search_mode = true;
        self.query.clear();
        self.apply_filter();
    }

    pub fn end_search(&mut self) {
        self.search_mode = false;
    }

    pub fn apply_filter(&mut self) {
        let needle = self.query.to_ascii_lowercase();
        self.filtered_indices = self
            .all_parameters
            .iter()
            .enumerate()
            .filter_map(|(i, value)| {
                if needle.is_empty() || value.name.to_ascii_lowercase().contains(&needle) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        if self.selected >= self.filtered_indices.len() {
            self.selected = 0;
        }
    }

    pub fn selected_parameter(&self) -> Option<&Parameter> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|idx| self.all_parameters.get(*idx))
    }

    pub fn configured_region_owned(&self) -> Option<String> {
        let trimmed = self.aws_region.trim();
        if trimmed.is_empty() || trimmed == "default-chain" {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    pub fn set_value_for_name(&mut self, name: &str, value: String) {
        if let Some(param) = self.all_parameters.iter_mut().find(|p| p.name == name) {
            param.value = Some(value);
        }
    }

    pub fn start_create(&mut self) {
        self.create_mode = true;
        self.search_mode = false;
        self.create_name.clear();
        self.create_value.clear();
        self.create_field = CreateField::Name;
        self.create_name_cursor = 0;
        self.create_value_cursor = 0;
        self.create_value_mode = ValueEditorMode::Insert;
        self.status = String::from("Create parameter popup opened");
    }

    pub fn cancel_create(&mut self) {
        self.create_mode = false;
        self.status = String::from("Create parameter cancelled");
    }

    pub fn switch_create_field(&mut self) {
        self.create_field = match self.create_field {
            CreateField::Name => CreateField::Value,
            CreateField::Value => CreateField::Name,
        };
        if self.create_field == CreateField::Value {
            self.create_value_mode = ValueEditorMode::Insert;
        }
    }

    pub fn submit_create(&mut self) {
        let name = self.create_name.trim().to_string();
        if name.is_empty() {
            self.status = String::from("Create failed: parameter name cannot be empty");
            return;
        }

        let value = self.create_value.clone();
        let created_version = match create_parameter_in_ssm(&name, &value, self.configured_region_owned()) {
            Ok(v) => v,
            Err(err) => {
                self.status = format!("Create failed in SSM: {err}");
                return;
            }
        };

        if let Some(existing) = self.all_parameters.iter_mut().find(|p| p.name == name) {
            existing.value = Some(value);
            existing.meta.param_type = Some("String".to_string());
            existing.meta.version = Some(created_version);
            existing.meta.tier = Some("Standard".to_string());
            existing.meta.data_type = Some("text".to_string());
            existing.meta.last_modified_epoch = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            );
        } else {
            let new_index = self.all_parameters.len();
            self.all_parameters.push(Parameter {
                name: name.clone(),
                value: Some(value),
                meta: ParameterMeta {
                    param_type: Some("String".to_string()),
                    version: Some(created_version),
                    tier: Some("Standard".to_string()),
                    data_type: Some("text".to_string()),
                    last_modified_epoch: Some(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                    ),
                    ..Default::default()
                },
            });

            self.query.clear();
            self.apply_filter();
            if let Some(pos) = self.filtered_indices.iter().position(|idx| *idx == new_index) {
                self.selected = pos;
            }
        }

        self.query.clear();
        self.apply_filter();
        if let Some(pos) = self
            .filtered_indices
            .iter()
            .position(|idx| self.all_parameters[*idx].name == name)
        {
            self.selected = pos;
        }

        self.create_mode = false;
        self.status = format!("Created parameter {name} in SSM and local state");
    }

    pub fn request_value_for_name(&mut self, name: &str) {
        if self.pending_value_requests.contains(name) {
            return;
        }

        let has_value = self
            .all_parameters
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.value.as_ref())
            .is_some();
        if has_value {
            return;
        }

        let Some(pool) = &self.value_pool else {
            return;
        };

        if pool.request_tx.send(name.to_string()).is_ok() {
            self.pending_value_requests.insert(name.to_string());
        }
    }

    pub fn is_value_pending(&self, name: &str) -> bool {
        self.pending_value_requests.contains(name)
    }

    pub fn pump_value_updates(&mut self) {
        let Some(pool) = &self.value_pool else {
            return;
        };

        while let Ok(update) = pool.response_rx.try_recv() {
            self.pending_value_requests.remove(&update.name);
            if let Some(param) = self
                .all_parameters
                .iter_mut()
                .find(|param| param.name == update.name)
            {
                match update.value {
                    Ok(value) => param.value = Some(value),
                    Err(err) => param.value = Some(format!("<error: {err}>")),
                }
            }
        }
    }

    pub fn prefetch_near_selected(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }

        let total = self.filtered_indices.len();
        let start = self.selected.saturating_sub(4);
        let end = (self.selected + 4).min(total.saturating_sub(1));

        let names: Vec<String> = (start..=end)
            .map(|idx| self.filtered_indices[idx])
            .filter_map(|src_idx| {
                let param = &self.all_parameters[src_idx];
                if param.value.is_none() {
                    Some(param.name.clone())
                } else {
                    None
                }
            })
            .collect();

        for name in names {
            self.request_value_for_name(&name);
        }
    }

    pub fn start_full_refresh(&mut self) {
        if self.full_refresh_rx.is_some() {
            self.status = String::from("Full refresh is already running");
            return;
        }

        self.status = String::from("Refreshing all parameters and values in background...");
        let (tx, rx) = mpsc::channel::<Result<FullRefreshResult, String>>();
        let region = self.configured_region_owned();
        thread::spawn(move || {
            let result =
                load_all_parameters_from_ssm(region).map(|(parameters, count, thread_count)| {
                    FullRefreshResult {
                        parameters,
                        count,
                        thread_count,
                    }
                });
            let _ = tx.send(result);
        });
        self.full_refresh_rx = Some(rx);
    }

    pub fn pump_full_refresh_updates(&mut self) {
        let result_opt = match self.full_refresh_rx.as_ref() {
            Some(rx) => match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("refresh worker disconnected".to_string()))
                }
            },
            None => None,
        };

        let Some(result) = result_opt else {
            return;
        };

        self.full_refresh_rx = None;
        let selected_name = self.selected_parameter().map(|p| p.name.clone());

        match result {
            Ok(done) => {
                self.all_parameters = done.parameters;
                self.pending_value_requests.clear();
                self.value_pool =
                    start_value_worker_pool(done.thread_count, self.configured_region_owned()).ok();
                self.apply_filter();
                if let Some(name) = selected_name
                    && let Some(pos) = self
                        .filtered_indices
                        .iter()
                        .position(|idx| self.all_parameters[*idx].name == name)
                {
                    self.selected = pos;
                }
                self.status = format!(
                    "Refreshed {} parameters with values using {} workers",
                    done.count, done.thread_count
                );
            }
            Err(err) => {
                self.status = format!("Refresh failed: {err}");
            }
        }
    }
}
