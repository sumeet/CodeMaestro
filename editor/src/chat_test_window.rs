use itertools::Itertools;

pub struct ChatTestWindow {
    messages: Vec<Message>,
}

impl ChatTestWindow {
    pub fn new() -> Self {
        Self { messages: vec![] }
    }

    pub fn view(&self) -> String {
        self.messages
            .iter()
            .map(|msg| format!("<{}> {}", msg.sender, msg.text))
            .join("\n")
    }

    pub fn add_message(&mut self, sender: String, text: String) {
        self.messages.push(Message { sender, text })
    }
}

struct Message {
    sender: String,
    text: String,
}
