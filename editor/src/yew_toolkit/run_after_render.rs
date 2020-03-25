use stdweb::web::HtmlElement;
use yew::virtual_dom::VNode;
use yew::Properties;
use yew::{html, NodeRef};
use yew::{Component, ComponentLink, Html};

pub struct RunAfterRender {
    props: RunAfterRenderProps,
}

#[derive(Properties)]
pub struct RunAfterRenderProps {
    #[props(required)]
    pub node_ref: NodeRef,
    #[props(required)]
    pub run: Box<dyn Fn(&HtmlElement)>,
}

#[allow(unused_must_use)]
pub fn run<T: Component>(html: Html<T>, func: impl Fn(&HtmlElement) + 'static) -> Html<T> {
    let node_ref = match &html {
        VNode::VTag(box tag) => tag.node_ref.clone(),
        _ => panic!("this only works w/ tags"),
    };
    let func: Box<dyn Fn(&HtmlElement)> = Box::new(func);
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

    fn mounted(&mut self) -> bool {
        if let Some(element) = self.props.node_ref.try_into::<HtmlElement>() {
            (self.props.run)(&element);
        }
        // TODO: hmm might need to rerender from inside of mounted huh
        //
        // we'll see
        false
    }

    fn update(&mut self, _msg: Self::Message) -> bool {
        true
    }

    // couldn't figure out a way to get this to actually render html, so we render it separately
    // always. this is just for calling the JS callback
    fn view(&self) -> Html<Self> {
        html! {}
    }
}
