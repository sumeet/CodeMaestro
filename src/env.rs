pub struct ExecutionEnvironment {
    pub console: String,
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        return ExecutionEnvironment {
            console: String::new(),
        }
    }

    pub fn println(&mut self, ln: &str) {
        self.console.push_str(ln);
        self.console.push_str("\n")
    }
}

