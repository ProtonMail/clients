#[derive(Default)]
pub struct QuarkCommand {
    query: Vec<String>,
    value: Vec<String>,
}

impl QuarkCommand {
    pub fn query(mut self, k: impl ToString, v: impl ToString) -> Self {
        self.query.push(k.to_string());
        self.query.push(v.to_string());
        self
    }

    pub fn query_flag(mut self, k: impl ToString) -> Self {
        self.query.push(k.to_string());
        self
    }

    pub fn query_flag_if(self, cond: bool, k: impl ToString) -> Self {
        if cond { self.query_flag(k) } else { self }
    }

    pub fn query_if_some(self, k: impl ToString, v: Option<impl ToString>) -> Self {
        if let Some(v) = v {
            self.query(k, v)
        } else {
            self
        }
    }

    pub fn value(mut self, v: impl ToString) -> Self {
        self.value.push(v.to_string());
        self
    }

    pub fn as_command(&self) -> String {
        let mut items = Vec::new();

        for item in &self.query {
            items.push(item.to_owned());
        }

        if !self.value.is_empty() {
            items.push("--".to_owned());
            for value in &self.value {
                if value.contains(['{', '[', ' ', '"']) {
                    items.push(format!("'{}'", value.clone()));
                } else {
                    items.push(value.clone());
                }
            }
        }

        items.join(" ")
    }
}
