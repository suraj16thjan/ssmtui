use std::sync::mpsc;

#[derive(Clone)]
pub struct Parameter {
    pub name: String,
    pub value: Option<String>,
    pub meta: ParameterMeta,
}

#[derive(Clone, Default)]
pub struct ParameterMeta {
    pub param_type: Option<String>,
    pub version: Option<i64>,
    pub tier: Option<String>,
    pub data_type: Option<String>,
    pub key_id: Option<String>,
    pub last_modified_epoch: Option<i64>,
    pub description: Option<String>,
    pub last_modified_user: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CreateField {
    Name,
    Value,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ValueEditorMode {
    Insert,
    Normal,
}

pub struct ValueFetchResult {
    pub name: String,
    pub value: Result<String, String>,
}

pub struct ValueWorkerPool {
    pub request_tx: mpsc::Sender<String>,
    pub response_rx: mpsc::Receiver<ValueFetchResult>,
}

pub struct FullRefreshResult {
    pub parameters: Vec<Parameter>,
    pub count: usize,
    pub thread_count: usize,
}
