use stdweb::web::HtmlElement;
use yew::virtual_dom::VNode;
use yew::Properties;
use yew::{html, NodeRef};
use yew::{Component, ComponentLink, Html};

pub struct RunAfterRender {
    props: RunAfterRenderProps,
}

#[derive(Properties, Clone)]
pub struct RunAfterRenderProps {
    // XXX: not sure if i need these annotations or not, they weren't working after i upgraded yew
    // #[props(required)]
    pub node_ref: NodeRef,
    // #[props(required)]
    run: FunctionContainer,
}

#[allow(unused_must_use)]
pub fn run(html: Html, func: impl Fn(&HtmlElement) + 'static + Clone) -> Html {
    let node_ref = match &html {
        VNode::VTag(box tag) => tag.node_ref.clone(),
        _ => panic!("this only works w/ tags"),
    };
    let func = FunctionContainer { function: Box::new(func) };
    html! {
        <>
            {{ html }}
            <RunAfterRender node_ref=node_ref, run=func />
        </>
    }
}

impl Component for RunAfterRender {
    type Message = ();
    type Properties = RunAfterRenderProps;

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Self { props }
    }

    fn rendered(&mut self, first_render: bool) {
        if first_render {
            if let Some(element) = self.props.node_ref.cast::<HtmlElement>() {
                self.props.run.call(&element);
            }
        }
    }

    fn update(&mut self, _msg: Self::Message) -> bool {
        true
    }

    // couldn't figure out a way to get this to actually render html, so we render it separately
    // always. this is just for calling the JS callback
    fn view(&self) -> Html {
        html! {}
    }

    fn change(&mut self, _props: Self::Properties) -> bool {
        // this should probably be true eh?
        false
    }
}

// from https://github.com/rust-lang/rust/issues/24000#issuecomment-479425396
#[derive(Clone)]
pub struct FunctionContainer {
    function: Box<dyn Function>,
}

impl FunctionContainer {
    fn call(&self, html_element: &HtmlElement) {
        (self.function)(html_element);
    }
}

trait Function: Fn(&HtmlElement) {
    fn clone_boxed(&self) -> Box<dyn Function>;
}

impl<T> Function for T where T: 'static + Clone + Fn(&HtmlElement)
{
    fn clone_boxed(&self) -> Box<dyn Function> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Function> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}
