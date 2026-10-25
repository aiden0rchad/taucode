use super::*;
#[derive(Default)]
pub struct Turn {
    pub context: Value,
    pub started: Option<std::time::Instant>,
    pub first_token: bool,
    pub tools: BTreeMap<String, u64>,
    pub reads: BTreeMap<String, u64>,
    pub errors: u64,
    pub tool_stats: BTreeMap<String, (u64, u64)>,
    pub compacted: bool,
}
impl Turn {
    pub fn tool(&mut self, name: &str, args: &Value, failed: bool) {
        let hash = format!("{:x}", Sha256::digest(format!("{name}:{args}").as_bytes()));
        *self.tools.entry(hash).or_default() += 1;
        if ["read_file", "read", "Read"].contains(&name) {
            let path = args["path"]
                .as_str()
                .or(args["file_path"].as_str())
                .unwrap_or("");
            let hash = format!("{:x}", Sha256::digest(path.as_bytes()));
            if !path.is_empty() {*self.reads.entry(hash).or_default() += 1;}
        }
        let stats = self.tool_stats.entry(name.into()).or_default();
        stats.0 += 1;
        stats.1 += u64::from(failed);
        self.errors += u64::from(failed);
    }
    pub fn failed_tool(&mut self, name: &str) {
        self.errors += 1;
        self.tool_stats.entry(name.into()).or_default().1 += 1;
    }
    pub fn finish(self, metrics: Value, history: &mut BTreeMap<String, Vec<u64>>) -> Vec<Event> {
        let mut base = event("usage", "agent", "agent.turn", "Turn completed");
        base.context = self.context;
        base.metrics = metrics;
        base.metrics["toolCalls"] = json!(self.tools.values().sum::<u64>());
        base.metrics["errors"] = json!(self.errors);
        if let Some(start) = self.started {
            base.metrics["durationMs"] = json!(start.elapsed().as_millis() as u64);
        }
        let tokens = base.metrics["tokensIn"].as_u64().unwrap_or(0)
            + base.metrics["tokensOut"].as_u64().unwrap_or(0);
        let model = base.context["model"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned();
        let past = history.entry(model).or_default();
        let mut sorted = past.clone();
        sorted.sort_unstable();
        let high = tokens > 50_000 && !sorted.is_empty() && tokens > 2 * sorted[sorted.len() / 2];
        if tokens > 0 {
            past.push(tokens);
        }
        if past.len() > 100 {
            past.remove(0);
        }
        let flags = [
            ("agent.tool_loop", self.tools.values().any(|&n| n >= 3)),
            ("agent.reread", self.reads.values().any(|&n| n >= 4)),
            ("agent.high_tokens", high),
            ("agent.compaction", self.compacted),
        ];
        let mut out = vec![base.clone()];
        for (code, on) in flags {
            if on {
                let mut e = base.clone();
                e.code = code.into();
                e.message = code.into();
                e.kind = "warning".into();
                e.severity = "medium".into();
                out.push(e);
            }
        }
        for (tool, (count, errors)) in self.tool_stats {
            if count > 0 && errors as f64 / count as f64 > 0.2 {
                let mut e = base.clone();
                e.code = "agent.tool_errors".into();
                e.message = format!("High tool error rate: {tool}");
                e.kind = "warning".into();
                e.severity = "medium".into();
                e.detail = json!({"tool":tool});
                e.metrics["count"] = json!(count);
                e.metrics["errors"] = json!(errors);
                out.push(e);
            }
        }
        out
    }
}
