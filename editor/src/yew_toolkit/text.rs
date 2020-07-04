use yew::prelude::html;
use yew::Component;
use yew::ComponentLink;
use yew::Html;
use yew::Properties;

pub struct Text {
    props: TextProps,
}
#[derive(PartialEq, Properties, Clone)]
pub struct TextProps {
    pub text: String,
}

impl Component for Text {
    type Message = ();
    type Properties = TextProps;

    fn create(props: Self::Properties, _: ComponentLink<Self>) -> Self {
        Self { props }
    }

    fn update(&mut self, _: Self::Message) -> bool {
        false
    }

    // TODO: use yewtil if we can get it working
    fn change(&mut self, props: Self::Properties) -> bool {
        if self.props != props {
            self.props = props;
            true
        } else {
            false
        }
    }

    fn view(&self) -> Html {
        // forgot why we needed to do this, whoops, should've written a comment
        let text = self.props.text.replace(" ", " ");
        html! {
            <div style="padding: 0.2em;",>
                {
                    if text.is_empty() {
                        html! { <span>{" "}</span> }
                    } else {
                       symbolize_text(&text)
                    }
                }
            </div>
        }
    }
}

pub fn symbolize_text(text: &str) -> Html {
    html! {
        <span>
            { for text.chars().map(|char| {
                if is_in_symbol_range(char) {
                    html! {
                        <span style="display: inline-block; font-size: 57%; transform: translateY(-1px);",>
                          { char }
                        </span>
                    }
                } else {
                    html! {
                        <span>{ char }</span>
                    }
                }
            })}
        </span>
    }
}

fn is_in_symbol_range(c: char) -> bool {
    match c as u32 {
        0xf000..=0xf72f => true,
        _ => false,
    }
}
