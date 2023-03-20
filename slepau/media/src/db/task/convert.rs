use common::utils::{DbError, CACHE_FOLDER};
use image::imageops::FilterType;
use image::ImageFormat;
use log::info;
use media::MEDIA_FOLDER;
use std::path::PathBuf;

use crate::db::meta::FileMeta;
use crate::db::version::{Max, Version, VersionReference};

/// Does conversion, this is the function spawned
/// that actually does the conversion and updates accordingly
pub fn do_convert(
	_ref: VersionReference,
) -> Result<(VersionReference, FileMeta, PathBuf), (VersionReference, DbError)> {
	let out_folder = std::path::Path::new(CACHE_FOLDER.as_str());
	if !out_folder.exists() {
		std::fs::create_dir(out_folder).unwrap();
	}
	let path_in = std::path::Path::new(MEDIA_FOLDER.as_str()).join(_ref.filename_in());
	let path_out = out_folder.join(_ref.filename_out());

	// let data = std::fs::read(&path).map_err(|e| (_ref.clone(), DbError::NotFound))?;
	let meta = FileMeta::from_path(&path_in);
	let version: Version = (&_ref.version).into();

	if meta._type.starts_with("image") {
		// let mut format = image::guess_format(&data).unwrap();
		let mut format = ImageFormat::from_mime_type(meta._type.clone()).ok_or_else(|| {
			(
				_ref.clone(),
				DbError::from(format!("Unknown image type '{}'.", meta._type)),
			)
		})?;

		let mut img = image::load(
			std::io::BufReader::new(std::fs::File::open(path_in).ok().unwrap()),
			format.clone(),
		)
		.unwrap();

		if let Some(orientation) = meta.exif.and_then(|v| {
			v.to_exif()
				.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
				.cloned()
		}) {
			let v = orientation.value.get_uint(0).unwrap();
			if [2, 4].contains(&v) {
				img = img.fliph();
			} else if [5, 7].contains(&v) {
				img = img.flipv();
			}
			if [5, 6].contains(&v) {
				img = img.rotate90();
			}
			if [3, 4].contains(&v) {
				img = img.rotate180();
			}
			if [8, 7].contains(&v) {
				img = img.rotate270();
			}
		}

		if let Some(max) = version.max {
			let mut width = img.width() as f32;
			let mut height = img.height() as f32;

			match max {
				Max::Absolute(x, y) => {
					if let Some(x) = x {
						width = x as f32
					}
					if let Some(y) = y {
						height = y as f32
					}
				}
				Max::Area(area) => {
					let max_area = area as f32;
					let max_to_current = (max_area / (width * height)).sqrt();
					if max_to_current < 1. {
						width = width * max_to_current;
						height = height * max_to_current;
					}
				}
			}
			img = img.resize(width.round() as u32, height.round() as u32, FilterType::Triangle);
		}

		if let Some(_type) = version._type {
			format = ImageFormat::from_mime_type(_type.clone())
				.ok_or_else(|| (_ref.clone(), DbError::from(format!("Unknown image type '{}'.", _type))))?;
		}

		img.save_with_format(path_out.clone(), format).unwrap();

		let meta_out = FileMeta::from_path(&path_out);

		return Ok((_ref, meta_out, path_out));
	} else if meta._type.clone().starts_with("video") {
		info!("In {}, out {}", path_in.to_str().unwrap(), path_out.to_str().unwrap());

		let mut command = std::process::Command::new("ffmpeg");
		command.args(["-i", path_in.to_str().unwrap()]);

		if version._type.clone().map(|t| t.starts_with("image")) == Some(true) {
			// Export first frame as an image
			// ffmpeg -i inputfile -vf "select=eq(n\,0)" -c:v png output_image

			command.args(["-frames:v", "1", "-f", "image2"]);

			command.args([
				"-c:v",
				version
					._type
					.and_then(|t| t.split("/").last().map(|t| t.to_string()))
					.or(version.c_v)
					.unwrap_or_else(|| "webp".into())
					.as_str(),
			]);

			command.arg(path_out.to_str().unwrap());
			command.output().unwrap();
			// I was thinking of going over the output with the image conversion processor above but nah, too much work.
		
		} else {
			// Set video output options

			// Codec video
			// let c_v = version.c_v.or_else(|| Some("libsvtav1".into()));
			if let Some(c_v) = version.c_v {
				command.args(["-c:v", c_v.as_str()]);
			}
			// Codec audio
			if let Some(c_a) = version.c_a {
				command.args(["-c:a", c_a.as_str()]);
			}

			// Bitrate video
			let b_v = version.b_v.or_else(|| Some("1M".into()));
			if let Some(b_v) = b_v {
				command.args(["-b:v", b_v.as_str()]);
			}
			// Bitrate audio
			if let Some(b_a) = version.b_a {
				command.args(["-b:a", b_a.as_str()]);
			}

			let _type = version._type.or(Some(meta._type));
			if let Some(_type) = _type.and_then(|t| t.split("/").last().map(|t| t.to_string())) {
				command.args(["-f", _type.as_str()]);
			}

			command.arg(path_out.to_str().unwrap());
			command.output().unwrap();
		}

		let meta_out = FileMeta::from_path(&path_out);

		return Ok((_ref, meta_out, path_out));
	} else {
		return Err((
			_ref,
			format!("Can't convert from unknown type '{}'.", meta._type).into(),
		));
	}
}
