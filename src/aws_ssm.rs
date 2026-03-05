use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    thread,
};

use aws_config::BehaviorVersion;
use aws_sdk_ssm::Client as SsmClient;
use aws_sdk_ssm::types::{ParameterMetadata as AwsParameterMetadata, ParameterType};
use aws_types::region::Region;

use crate::models::{Parameter, ParameterMeta, ValueFetchResult, ValueWorkerPool};

pub fn load_parameter_names_from_ssm(
    region: Option<String>,
) -> Result<(Vec<Parameter>, usize, usize), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to initialize async runtime: {e}"))?;

    rt.block_on(async {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = region.clone() {
            loader = loader.region(Region::new(region));
        }
        let sdk_config = loader.load().await;
        let client = SsmClient::new(&sdk_config);

        let mut all_params = Vec::<Parameter>::new();
        let mut next_token: Option<String> = None;

        loop {
            let response = client
                .describe_parameters()
                .set_next_token(next_token.clone())
                .max_results(50)
                .send()
                .await
                .map_err(|e| format!("describe_parameters failed: {e}"))?;

            if let Some(parameters) = response.parameters {
                for param in parameters {
                    let meta = metadata_from_ssm(&param);
                    if let Some(name) = param.name {
                        all_params.push(Parameter {
                            name,
                            value: None,
                            meta,
                        });
                    }
                }
            }

            next_token = response.next_token;
            if next_token.is_none() {
                break;
            }
        }

        all_params.sort_by(|a, b| a.name.cmp(&b.name));
        let count = all_params.len();
        let thread_count = count / 30 + 1;

        Ok((all_params, count, thread_count))
    })
}

pub fn load_all_parameters_from_ssm(
    region: Option<String>,
) -> Result<(Vec<Parameter>, usize, usize), String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to initialize async runtime: {e}"))?;

    rt.block_on(async {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = region.clone() {
            loader = loader.region(Region::new(region));
        }
        let sdk_config = loader.load().await;
        let client = SsmClient::new(&sdk_config);

        let mut names = Vec::<String>::new();
        let mut meta_by_name = HashMap::<String, ParameterMeta>::new();
        let mut next_token: Option<String> = None;

        loop {
            let response = client
                .describe_parameters()
                .set_next_token(next_token.clone())
                .max_results(50)
                .send()
                .await
                .map_err(|e| format!("describe_parameters failed: {e}"))?;

            if let Some(parameters) = response.parameters {
                for param in parameters {
                    let meta = metadata_from_ssm(&param);
                    if let Some(name) = param.name {
                        meta_by_name.insert(name.clone(), meta);
                        names.push(name);
                    }
                }
            }

            next_token = response.next_token;
            if next_token.is_none() {
                break;
            }
        }

        names.sort();
        let count = names.len();
        let thread_count = count / 30 + 1;
        let ordered_names = names.clone();

        let mut buckets = vec![Vec::<String>::new(); thread_count];
        for (idx, name) in names.into_iter().enumerate() {
            buckets[idx % thread_count].push(name);
        }

        let mut handles = Vec::with_capacity(thread_count);
        for bucket in buckets {
            let worker_sdk_config = sdk_config.clone();
            let handle = thread::spawn(move || -> Result<Vec<(String, String)>, String> {
                let worker_rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| format!("worker runtime init failed: {e}"))?;

                worker_rt.block_on(async move {
                    let worker_client = SsmClient::new(&worker_sdk_config);
                    let mut params = Vec::<(String, String)>::new();

                    for chunk in bucket.chunks(10) {
                        if chunk.is_empty() {
                            continue;
                        }
                        let names = chunk.to_vec();
                        let response = match worker_client
                            .get_parameters()
                            .set_names(Some(names.clone()))
                            .with_decryption(true)
                            .send()
                            .await
                        {
                            Ok(resp) => resp,
                            Err(_) => worker_client
                                .get_parameters()
                                .set_names(Some(names))
                                .with_decryption(false)
                                .send()
                                .await
                                .map_err(|e| format!("get_parameters failed: {e}"))?,
                        };

                        if let Some(parameters) = response.parameters {
                            for parameter in parameters {
                                if let Some(name) = parameter.name {
                                    params.push((name, parameter.value.unwrap_or_default()));
                                }
                            }
                        }
                    }

                    Ok(params)
                })
            });
            handles.push(handle);
        }

        let mut value_by_name = HashMap::<String, String>::new();
        for handle in handles {
            let result = handle
                .join()
                .map_err(|_| "refresh worker thread panicked".to_string())?;
            for (name, value) in result? {
                value_by_name.insert(name, value);
            }
        }

        let mut merged = Vec::<Parameter>::with_capacity(ordered_names.len());
        for name in ordered_names {
            let meta = meta_by_name.remove(&name).unwrap_or_default();
            let value = value_by_name.remove(&name);
            merged.push(Parameter { name, value, meta });
        }
        Ok((merged, count, thread_count))
    })
}

fn metadata_from_ssm(param: &AwsParameterMetadata) -> ParameterMeta {
    ParameterMeta {
        param_type: param.r#type.as_ref().map(|t| t.as_str().to_string()),
        version: Some(param.version),
        tier: param.tier.as_ref().map(|t| t.as_str().to_string()),
        data_type: param.data_type.clone(),
        key_id: param.key_id.clone(),
        last_modified_epoch: param.last_modified_date.as_ref().map(|d| d.secs()),
        description: param.description.clone(),
        last_modified_user: param.last_modified_user.clone(),
    }
}

pub fn start_value_worker_pool(
    thread_count: usize,
    region: Option<String>,
) -> Result<ValueWorkerPool, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to initialize async runtime: {e}"))?;

    let sdk_config = rt.block_on(async {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = region {
            loader = loader.region(Region::new(region));
        }
        loader.load().await
    });

    let (request_tx, request_rx) = mpsc::channel::<String>();
    let (response_tx, response_rx) = mpsc::channel::<ValueFetchResult>();
    let shared_request_rx = Arc::new(Mutex::new(request_rx));

    for _ in 0..thread_count {
        let worker_sdk_config = sdk_config.clone();
        let worker_request_rx = Arc::clone(&shared_request_rx);
        let worker_response_tx = response_tx.clone();

        thread::spawn(move || {
            let worker_rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };

            let client = SsmClient::new(&worker_sdk_config);

            loop {
                let name = {
                    let guard = match worker_request_rx.lock() {
                        Ok(guard) => guard,
                        Err(_) => return,
                    };
                    match guard.recv() {
                        Ok(name) => name,
                        Err(_) => break,
                    }
                };

                let value = worker_rt.block_on(fetch_single_parameter_value(&client, &name));
                let _ = worker_response_tx.send(ValueFetchResult { name, value });
            }
        });
    }

    Ok(ValueWorkerPool {
        request_tx,
        response_rx,
    })
}

async fn fetch_single_parameter_value(client: &SsmClient, name: &str) -> Result<String, String> {
    let response = match client
        .get_parameter()
        .name(name)
        .with_decryption(true)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(_) => client
            .get_parameter()
            .name(name)
            .with_decryption(false)
            .send()
            .await
            .map_err(|e| format!("get_parameter failed: {e}"))?,
    };

    let parameter = response
        .parameter
        .ok_or_else(|| "parameter missing in get_parameter response".to_string())?;

    Ok(parameter.value.unwrap_or_default())
}

pub fn fetch_parameter_value_from_ssm(name: &str, region: Option<String>) -> Result<String, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to initialize async runtime: {e}"))?;

    rt.block_on(async {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = region {
            loader = loader.region(Region::new(region));
        }
        let sdk_config = loader.load().await;
        let client = SsmClient::new(&sdk_config);
        fetch_single_parameter_value(&client, name).await
    })
}

fn map_parameter_type(type_name: Option<&str>) -> Option<ParameterType> {
    match type_name {
        Some("String") => Some(ParameterType::String),
        Some("StringList") => Some(ParameterType::StringList),
        Some("SecureString") => Some(ParameterType::SecureString),
        _ => Some(ParameterType::String),
    }
}

pub fn put_parameter_value_to_ssm(
    name: &str,
    value: &str,
    meta: &ParameterMeta,
    region: Option<String>,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to initialize async runtime: {e}"))?;

    rt.block_on(async {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = region {
            loader = loader.region(Region::new(region));
        }
        let sdk_config = loader.load().await;
        let client = SsmClient::new(&sdk_config);

        let mut request = client
            .put_parameter()
            .name(name)
            .value(value)
            .overwrite(true)
            .set_type(map_parameter_type(meta.param_type.as_deref()));

        if matches!(meta.param_type.as_deref(), Some("SecureString")) {
            request = request.set_key_id(meta.key_id.clone());
        }

        request
            .send()
            .await
            .map_err(|e| format!("put_parameter failed: {e}"))?;
        Ok(())
    })
}

pub fn create_parameter_in_ssm(name: &str, value: &str, region: Option<String>) -> Result<i64, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("failed to initialize async runtime: {e}"))?;

    rt.block_on(async {
        let mut loader = aws_config::defaults(BehaviorVersion::latest());
        if let Some(region) = region {
            loader = loader.region(Region::new(region));
        }
        let sdk_config = loader.load().await;
        let client = SsmClient::new(&sdk_config);
        let output = client
            .put_parameter()
            .name(name)
            .value(value)
            .set_type(Some(ParameterType::String))
            .overwrite(false)
            .send()
            .await
            .map_err(|e| format!("put_parameter(create) failed: {e}"))?;
        Ok(output.version)
    })
}
