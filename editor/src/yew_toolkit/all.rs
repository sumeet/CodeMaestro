use yew::html::Children;
use yew::prelude::html;
use yew::Component;
use yew::ComponentLink;
use yew::Html;
use yew::Properties;

pub struct All {
    props: AllProps,
}

#[derive(PartialEq, Properties, Clone)]
pub struct AllProps {
    pub children: Children,
}

impl Component for All {
    type Message = ();
    type Properties = AllProps;

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        All { props }
    }

    // TODO: do we need to do something here to get messages propagated up from children?
    fn update(&mut self, _msg: Self::Message) -> bool {
        false
    }

    fn change(&mut self, props: Self::Properties) -> bool {
        let mut props = props;
        std::mem::swap(&mut self.props, &mut props);
        props != self.props
    }

    fn view(&self) -> Html {
        html! {
            <div class="all-drawn", style="display: flex; flex-direction: column;",>
                {self.props.children.clone()}
            </div>
        }
    }
}
