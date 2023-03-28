use lazy_static::lazy_static;
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;

use common::utils::{REGEX_ACCESS, REGEX_TITLE};

fn md_to_html(value: &str) -> String {
	let mut options = Options::empty();
	options.insert(Options::ENABLE_STRIKETHROUGH);
	let parser = Parser::new_ext(value, options);

	let mut html_output = String::new();
	html::push_html(&mut html_output, parser);
	html_output
}

pub fn value_transform(value: &str) -> String {
	lazy_static! {
		static ref REPLACEMENTS: [(Regex, &'static str); 9] = [
			(REGEX_TITLE.clone(), "# $1\n"),
			(Regex::new(r"\[ \]").unwrap(), "&#x2610;"),
			(Regex::new(r"\[[xX]\]").unwrap(), "&#x2612;"),
			(Regex::new(r"\[check\]").unwrap(), "&#x2713;"),
			(REGEX_ACCESS.clone(), ""),
			(
				Regex::new(concat!(r"\(media/(", env!("REGEX_PROQUINT"), r")\)")).unwrap(),
				"(/media/$1)"
			),
			(
				Regex::new(concat!(r"\(image/(", env!("REGEX_PROQUINT"), r")\)")).unwrap(),
				r#"<img
	style="display: block"
	src="/media/$1?max=800x"
	srcset="
		/media/$1?max=480x   480w,
		/media/$1?max=800x   800w,
		/media/$1?max=1200x 1200w,
		/media/$1?max=x 1600w
	"
	sizes="(min-width:800px) 800px, 100vw"
/>"#
			),
			(
				Regex::new(concat!(r"\(video/(", env!("REGEX_PROQUINT"), r")\)")).unwrap(),
				r#"<video controls> 
<source src="/media/$1?type=video/webm" type="video/webm" />
<source src="/media/$1?c_v=libx264&c_a=aac&b_v=2M&b_a=90k&type=video/mp4" type="video/mp4" />Your browser doesn't support HTML video. Click to download <a href="/media/$1?c_v=libx264&c_a=aac&b_v=2M&b_a=90k&type=video/mp4">$1</a> instead.</video>"#
			),
			(
				Regex::new(concat!(r"\(chunks?/(", env!("REGEX_PROQUINT"), r")\)")).unwrap(),
				"(/page/$1)"
			),
		];
	}

	let mut value = value.to_string();
	for (regex, rep) in REPLACEMENTS.as_ref() {
		value = regex.replace_all(&value, *rep).to_string();
	}

	value
}
pub fn value_to_html(value: &str) -> String {
	md_to_html(&value_transform(value))
}
