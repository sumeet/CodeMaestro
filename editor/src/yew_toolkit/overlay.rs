use yew::html;
use yew::Children;
use yew::Component;
use yew::ComponentLink;
use yew::Html;
use yew::Properties;

pub struct TopRightOverlay {
    props: TopRightOverlayProps,
}

#[derive(PartialEq, Properties, Clone)]
pub struct TopRightOverlayProps {
    pub top_position_px: u32,
    pub right_position_px: u32,
    pub background_color: String,
    pub children: Children,
}

impl Component for TopRightOverlay {
    type Message = ();
    type Properties = TopRightOverlayProps;

    fn create(props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        TopRightOverlay { props }
    }

    fn update(&mut self, _msg: Self::Message) -> bool {
        false
    }

    fn change(&mut self, props: Self::Properties) -> bool {
        let mut props = props;
        std::mem::swap(&mut self.props, &mut props);
        props != self.props
    }

    fn view(&self) -> Html {
        let style = format!("padding: 0.5em; position: absolute; top: {}px; right: {}px; color: white; background-color: {}",
                            self.props.top_position_px, self.props.right_position_px, self.props.background_color);
        html! {
            <div class="window-border", style=style, >
                {self.props.children.clone()}
            </div>
        }
    }
}

pub struct TopLeftOverlay {
    props: TopLeftOverlayProps,
}

#[derive(PartialEq, Properties, Clone)]
pub struct TopLeftOverlayProps {
    pub top_position_px: u32,
    pub left_position_px: u32,
    pub background_color: String,
    pub children: Children,
}

impl Component for TopLeftOverlay {
    type Message = ();
    type Properties = TopLeftOverlayProps;

    fn create(props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        TopLeftOverlay { props }
    }

    fn update(&mut self, _msg: Self::Message) -> bool {
        false
    }

    fn change(&mut self, props: Self::Properties) -> bool {
        let mut props = props;
        std::mem::swap(&mut self.props, &mut props);
        props != self.props
    }

    fn view(&self) -> Html {
        let style = format!("padding: 0.5em; position: absolute; top: {}px; left: {}px; color: white; background-color: {}",
            self.props.top_position_px, self.props.left_position_px, self.props.background_color);
        html! {
            <div class="window-border", style=style, >
                {self.props.children.clone()}
            </div>
        }
    }
}
