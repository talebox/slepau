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
		static ref REPLACEMENTS: [(Regex, &'static str); 7] = [
			(REGEX_TITLE.clone(), "# $1\n"),
			(Regex::new(r"\[ \]").unwrap(), "&#x2610;"),
			(Regex::new(r"\[[xX]\]").unwrap(), "&#x2612;"),
			(Regex::new(r"\[check\]").unwrap(), "&#x2713;"),
			(REGEX_ACCESS.clone(), ""),
			(
				Regex::new(concat!(r"\(media/(", env!("REGEX_PROQUINT"), r")\)")).unwrap(),
				"(/api/media/$1)"
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
