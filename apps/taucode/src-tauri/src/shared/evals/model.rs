use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Task {
    pub id: String, pub repo: String, pub category: String, pub tags: Vec<String>,
    pub prompt: String, pub setup: Vec<String>, pub check: Check,
    pub extra_checks: Vec<Extra>, pub answer_contains: Vec<String>, pub requires: Vec<String>,
    pub timeout_sec: u64, pub max_turns: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Check { pub command: String, pub expect_exit: i32 }
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extra { pub file: String, pub contains: String }
#[derive(Deserialize)]
pub struct Suite { pub tasks: Vec<Task>, pub repos: BTreeMap<String, BTreeMap<String,String>> }
pub fn suite() -> Suite { serde_json::from_str(include_str!("../../../../evals/catalog.json")).expect("embedded eval suite") }
fn one() -> usize { 1 }
fn two() -> usize { 2 }
fn off() -> Vec<bool> { vec![false] }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub struct Plan {
    #[serde(default)] pub ids: Vec<String>,
    #[serde(default)] pub tags: Vec<String>,
    #[serde(default)] pub categories: Vec<String>,
    pub engines: Vec<String>, pub models: Vec<String>,
    pub effort: Option<String>,
    #[serde(default="off")] pub codegraph: Vec<bool>,
    #[serde(default="one")] pub repeats: usize,
    #[serde(default="two")] pub concurrency: usize,
    pub max_total_tokens: Option<u64>, pub max_cost: Option<f64>,
    pub keep_workdirs: Option<bool>,
}
impl Plan {
    pub fn tasks(&self) -> Vec<Task> {
        suite().tasks.into_iter().filter(|t|
            (self.ids.is_empty() || self.ids.contains(&t.id)) &&
            (self.tags.is_empty() || self.tags.iter().any(|tag| t.tags.contains(tag))) &&
            (self.categories.is_empty() || self.categories.contains(&t.category))).collect()
    }
    pub fn validate(&self) -> Result<(),String> {
        if self.engines.is_empty() || self.models.is_empty() || self.codegraph.is_empty() || !(1..=6).contains(&self.concurrency) || !(1..=100).contains(&self.repeats) { return Err("Choose engines/models, 1–100 repeats and concurrency 1–6".into()); }
        if self.engines.iter().collect::<std::collections::HashSet<_>>().len()!=self.engines.len()
            || self.models.iter().collect::<std::collections::HashSet<_>>().len()!=self.models.len()
            || self.codegraph.iter().collect::<std::collections::HashSet<_>>().len()!=self.codegraph.len() { return Err("Matrix axes must not contain duplicates".into()); }
        if self.models.iter().any(|m| !m.split_once('/').is_some_and(|(a,b)| !a.is_empty() && !b.is_empty())) { return Err("Models must be provider/model references".into()); }
        if self.effort.as_deref().is_some_and(|level| !super::super::reasoning::LEVELS.contains(&level)) { return Err("Unknown reasoning level".into()); }
        if self.max_cost.is_some_and(|n| !n.is_finite() || n <= 0.) || self.max_total_tokens == Some(0) { return Err("Budgets must be positive".into()); }
        if self.tasks().is_empty() || self.tasks().len()*self.engines.len()*self.models.len()*self.codegraph.len()*self.repeats > 10000 { return Err("Choose between 1 and 10000 cells".into()); }
        if self.ids.iter().any(|id| !suite().tasks.iter().any(|t| &t.id == id)) { return Err("Unknown task ID".into()); }
        Ok(())
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Usage {
    pub tokens_in: u64, pub cached_tokens: u64, pub tokens_out: u64, pub cost_usd: Option<f64>,
    pub tool_calls: u64, pub errors: Vec<String>, pub engine_ready_ms: u64,
    pub first_token_ms: Option<u64>, pub wall_ms: u64,
}
impl Usage { pub fn tokens(&self) -> u64 { self.tokens_in + self.cached_tokens + self.tokens_out } }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Cell {
    pub id: String, pub task: String, pub engine: String, pub model: String,
    pub codegraph: bool, pub repeat: usize, pub status: String, pub turns: usize,
    pub thread_id: Option<String>, pub output: String, pub answer: String, pub usage: Usage,
    pub transcript: Vec<Value>, pub workdir: Option<String>,
    #[serde(default)] pub completed_at: Option<String>,
}
impl Cell { pub fn config(&self) -> String { format!("{}/{}/{}",self.engine,self.model,self.codegraph) } }
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct Run {
    pub id: String, pub created_at: String, pub status: String, pub plan: Plan,
    #[serde(default)] pub message: Option<String>,
    pub environment: Value, pub cells: Vec<Cell>,
}
pub fn safe_id(id: &str) -> Result<(),String> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') { Err("Invalid eval ID".into()) } else { Ok(()) }
}
pub fn inside(root: &Path, name: &str) -> bool {
    let path = root.join(name);
    let Ok(canonical) = root.canonicalize() else { return false };
    // Accept the root as given or resolved (/var vs /private/var); the canonical check below confines it.
    if !(path.starts_with(root) || path.starts_with(&canonical)) || path.components().any(|c| c == std::path::Component::ParentDir) { return false; }
    let mut ancestor = path.as_path();
    while !ancestor.exists() { let Some(parent) = ancestor.parent() else { return false }; ancestor=parent; }
    ancestor.canonicalize().is_ok_and(|p| p.starts_with(canonical))
}
pub fn approval(root: &Path, params: &Value) -> bool {
    // Opaque commands cannot be proven confined, even when cwd is inside the repo.
    fn collect<'a>(v: &'a Value, paths: &mut Vec<&'a str>, targets: &mut usize) -> bool {
        if let Some(obj)=v.as_object() { for (key,value) in obj {
            if matches!(key.as_str(),"command"|"commandActions"|"commandLine"|"shell") && !value.is_null() { return false; }
            if matches!(key.as_str(),"path"|"filePath"|"file_path"|"targetPath"|"file"|"directory"|"cwd") {
                let Some(path)=value.as_str() else { return false };
                paths.push(path); if key!="cwd" { *targets+=1; }
            } else if key=="changes" && value.is_object() {
                for path in value.as_object().unwrap().keys() { paths.push(path); *targets+=1; }
                if !collect(value,paths,targets) { return false; }
            } else if !collect(value,paths,targets) { return false; }
        }} else if let Some(items)=v.as_array() {
            for item in items { if !collect(item,paths,targets) { return false; } }
        } else if let Some(text)=v.as_str() {
            if Path::new(text).is_absolute() || text.starts_with("../") { paths.push(text); }
        }
        true
    }
    let mut paths=Vec::new(); let mut targets=0;
    collect(params,&mut paths,&mut targets) && targets>0 && paths.iter().all(|p|inside(root,p))
}
pub fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() { return 0.; } values.sort_by(f64::total_cmp);
    let mid=values.len()/2; if values.len()%2==0 { (values[mid-1]+values[mid])/2. } else { values[mid] }
}
pub fn summary(run: &Run) -> Vec<Value> {
    let mut groups: BTreeMap<String,Vec<&Cell>>=BTreeMap::new();
    for cell in &run.cells { if !matches!(cell.status.as_str(),"skipped"|"queued"|"running"|"cancelled") { groups.entry(cell.config()).or_default().push(cell); } }
    groups.into_iter().map(|(config,cells)| {
        let input=cells.iter().map(|c| c.usage.tokens_in+c.usage.cached_tokens).sum::<u64>();
        json!({"config":config,"engine":cells[0].engine,"model":cells[0].model,"codegraph":cells[0].codegraph,
            "passRate":cells.iter().filter(|c|c.status=="passed").count() as f64/cells.len() as f64,
            "tokens":median(cells.iter().map(|c|c.usage.tokens() as f64).collect()),
            "cacheRate":if input==0 {0.} else {cells.iter().map(|c|c.usage.cached_tokens).sum::<u64>() as f64/input as f64},
            "costUsd":cells.iter().try_fold(0.,|sum,c|c.usage.cost_usd.map(|n|sum+n)),
            "firstTokenMs":if cells.iter().any(|c|c.usage.first_token_ms.is_some()) {Some(median(cells.iter().filter_map(|c|c.usage.first_token_ms.map(|n|n as f64)).collect()))} else {None},
            "wallMs":median(cells.iter().map(|c|c.usage.wall_ms as f64).collect()),
            "toolCalls":cells.iter().map(|c|c.usage.tool_calls).sum::<u64>(),
            "errors":cells.iter().map(|c|c.usage.errors.len()).sum::<usize>()})
    }).collect()
}
pub fn gate(baseline: &Run, candidate: &Run, drop: f64, increase: f64) -> Result<Value,String> {
    if !drop.is_finite() || !(0. ..=1.).contains(&drop) || !increase.is_finite() || increase < 0. { return Err("Invalid regression thresholds".into()); }
    if baseline.status!="completed" || candidate.status!="completed" { return Err("Gate requires completed runs".into()); }
    let keys=|r:&Run| r.cells.iter().filter(|c|c.status!="skipped").map(|c|format!("{}/{}/{}",c.config(),c.task,c.repeat)).collect::<std::collections::BTreeSet<_>>();
    if keys(baseline).is_empty() || keys(baseline)!=keys(candidate) { return Err("Gate requires matching tasks, configurations and repeats".into()); }
    let old=summary(baseline); let new=summary(candidate);
    let rows: Vec<_>=new.iter().map(|n| {
        let b=old.iter().find(|b| b["config"]==n["config"]).unwrap();
        let pass_drop=b["passRate"].as_f64().unwrap()-n["passRate"].as_f64().unwrap();
        let previous=b["tokens"].as_f64().unwrap(); let current=n["tokens"].as_f64().unwrap();
        let token_regression=current>previous*(1.+increase);
        json!({"config":n["config"],"passRate":n["passRate"],"tokens":n["tokens"],"passDrop":pass_drop,
            "tokenIncrease":if previous==0. {None} else {Some(current/previous-1.)},
            "regressed":pass_drop>drop || token_regression})
    }).collect();
    Ok(json!({"passed":!rows.iter().any(|r|r["regressed"]==true),"rows":rows}))
}

