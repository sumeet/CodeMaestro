use std::rc::Rc;
extern crate cs;

fn main() {
    let app = Rc::new(cs::CSApp::new());
    cs::draw_app(app);
}
