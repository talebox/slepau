use std::collections::HashSet;

use serde_json::{json, Value};

use crate::db::{
	chunk::Chunk,
	view::{ChunkId, ChunkView, ViewType},
};

use super::{dbchunk::DBChunk, GraphView, DB};

#[test]
fn delete() {
	let mut db = DB::default();

	let c_notes: DBChunk = "# Notes\n".into();
	let id_notes = c_notes.chunk().id.clone();
	assert!(db.set_chunk(c_notes, "john").is_ok());
	assert_eq!(
		db.del_chunk([id_notes].into(), "john"),
		Ok(HashSet::from(["john".into()]))
	);
}
#[test]
fn sharing() {
	let mut db = DB::default();

	let c_notes: DBChunk = "# Notes\nshare: poca w, nina a".into();
	println!("{:?}", c_notes.props());
	let id_notes = c_notes.chunk().id.clone();
	assert!(db.set_chunk(c_notes, "john").is_ok());

	assert_eq!(
		db.set_chunk(
			(id_notes.as_str(), "# Notes\nHello :)\nshare: poca w, nina a").into(),
			"poca"
		),
		Ok(HashSet::default())
	);
	assert!(db
		.set_chunk((id_notes.as_str(), "# Notes\nshare: poca w, nina r").into(), "poca")
		.is_err());
	// let c_notes: DBChunk = "# Notes\nHello :)\nshare: poca w, nina a".into();
	// println!("{:?}", c_notes.props());
	assert!(db
		.set_chunk(
			(id_notes.as_str(), "# Notes\nHello :)\nshare: poca w, nina a").into(),
			"nina"
		)
		.is_ok());
	assert!(db
		.set_chunk((id_notes.as_str(), "# Notes\nHello :)\nshare: nina a").into(), "nina")
		.is_ok());
	assert!(db
		.set_chunk(
			(id_notes.as_str(), "# Notes\nHello :)\nshare: poca rnina a").into(),
			"nina"
		)
		.is_err()); // Errors out because nina would be deleting her own admin access
	assert!(db
		.set_chunk(
			(id_notes.as_str(), "# Notes\nHello :)\nshare: poca r,nina a").into(),
			"nina"
		)
		.is_ok());
	assert!(db.del_chunk(HashSet::from([id_notes]), "nina").is_ok()); // Nina can delete as well
}
// Make sure users can't see public documents in their views
#[test]
fn visibility() {
	let mut db = DB::default();

	// John creates a chunk
	let c_notes: DBChunk = "# Notes\nshare: public a, public r".into();
	let id_notes = c_notes.chunk().id.clone();
	assert!(db.set_chunk(c_notes, "john").is_ok());
	{
		// Test visibility

		assert_eq!(db.get_chunks("nina").len(), 0); // Nina can't see anything
		assert_eq!(db.get_chunks("john").len(), 1); // John can see his own
		assert_eq!(db.get_chunks("public").len(), 0); // Public can't see anything

		assert_eq!(
			db.subtree(None, &"nina".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1),
			GraphView(Value::Null, Some(vec![]))
		); // Nina can't see anything
		assert_eq!(
			db.subtree(None, &"john".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1),
			GraphView(Value::Null, Some(vec![GraphView(json!(id_notes), None)]))
		); // John can see his own
		assert_eq!(
			db.subtree(None, &"public".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1),
			GraphView(Value::Null, None)
		); // Public can't see anything
	}

	// Public tries creating a chunk but is denied
	let c_notes: DBChunk = "# Notes 2\nshare: public a, john a".into();
	assert!(db.set_chunk(c_notes, "public").is_err());
	// Nina creates a chunk, giving john admin access
	let c_notes: DBChunk = "# Notes 2\nshare: public a, john a".into();
	let id_notes2 = c_notes.chunk().id.clone();
	assert!(db.set_chunk(c_notes, "nina").is_ok());

	{
		// Test visibility

		assert_eq!(db.get_chunks("nina").len(), 1);
		assert_eq!(db.get_chunks("john").len(), 2);
		assert_eq!(db.get_chunks("public").len(), 0); // Public can't see anything

		assert_eq!(
			db.subtree(None, &"nina".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1),
			GraphView(Value::Null, Some(vec![GraphView(json!(id_notes2), None)]))
		);
		assert_eq!(
			db.subtree(None, &"john".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1)
				.1
				.unwrap()
				.len(),
			2
		);
		assert_eq!(
			db.subtree(None, &"public".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1),
			GraphView(Value::Null, None)
		); // Public can't see anything
	}

	// Nina creates another chunk, giving john also admin access
	let c_notes: DBChunk = format!("# Notes 3 -> {id_notes2}\nshare: public a, john a")
		.as_str()
		.into();
	let id_notes3 = c_notes.chunk().id.clone();
	assert!(db.set_chunk(c_notes, "nina").is_ok());

	{
		// Test visibility

		assert_eq!(db.get_chunks("nina").len(), 2);
		assert_eq!(db.get_chunks("john").len(), 3);
		assert_eq!(db.get_chunks("public").len(), 0); // Public can't see anything

		assert_eq!(
			db.subtree(None, &"nina".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1),
			GraphView(Value::Null, Some(vec![GraphView(json!(id_notes2), None)]))
		);
		assert_eq!(
			db.subtree(None, &"nina".into(), &|v| v, &|v| json!(ChunkId::from(v)), 2),
			GraphView(
				Value::Null,
				Some(vec![GraphView(
					json!(id_notes2),
					Some(vec![GraphView(json!(id_notes3), None)])
				)])
			)
		);
		assert_eq!(
			db.subtree(None, &"john".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1)
				.1
				.unwrap()
				.len(),
			2
		);
		assert_eq!(
			db.subtree(None, &"public".into(), &|v| v, &|v| json!(ChunkId::from(v)), 1),
			GraphView(Value::Null, None)
		); // Public can't see anything
	}
}
/// Create a "Notes"
/// Modify "Notes" 10 sec after
/// Assert that Modify is 10 sec after Created.
#[test]
fn created_modified() {
	let mut db = DB::default();

	let c_notes: DBChunk = "# Notes\n".into();
	let cre_notes = c_notes.chunk().created;

	let id_notes = c_notes.chunk().id.clone();
	db.set_chunk(c_notes, "john").unwrap();

	let mut c_notes: Chunk = (id_notes.as_str(), "# Notes\n").into();
	c_notes.created += 10;
	c_notes.modified += 10;
	let mod_notes = c_notes.modified;
	db.set_chunk(c_notes.into(), "john").unwrap();

	let notes = db.get_chunk(&id_notes, "john").unwrap();
	{
		let chunk_notes = notes.read().unwrap();
		assert_eq!(chunk_notes.chunk().created, cre_notes);
		assert_eq!(chunk_notes.chunk().modified, mod_notes);
	}

	{
		let view = ChunkView::from((notes, "john", ViewType::Edit));
		assert_eq!(view.created, Some(cre_notes));
		assert_eq!(view.modified, Some(mod_notes));
	}
}
/// Create a "Notes"
/// Create a "Note1 -> Notes" with modified 10 sec after
/// Assert that Dynamic Modified on Notes = Note1's modify time (10 sec after)
#[test]
fn dynamic_modified() {
	let mut db = DB::default();
	let c_notes: DBChunk = "# Notes\n".into();
	let mod_notes = c_notes.chunk().modified;
	let id_notes = c_notes.chunk().id.clone();
	db.set_chunk(c_notes, "john").unwrap();

	let mut chunk_note1: Chunk = format!("# Note 1 -> {}\n", &id_notes).as_str().into();
	let mod_note1 = mod_notes + 10;
	chunk_note1.modified = mod_note1;
	let c_note1 = DBChunk::from(chunk_note1);
	let _id_note1 = c_note1.chunk().id.clone();

	assert!(db.set_chunk(c_note1, "john").is_ok());

	assert_eq!(
		db.get_chunk(&id_notes, "john")
			.unwrap()
			.write()
			.unwrap()
			.get_prop_dynamic::<u64>("modified", &"john".into())
			.unwrap(),
		mod_note1
	);
}
#[test]
fn well() {
	let mut db = DB::default();

	let c_notes: DBChunk = "# Notes\n".into();
	let id_notes = c_notes.chunk().id.clone();
	assert_eq!(
		db.set_chunk(c_notes, "john"),
		Ok(HashSet::from(["john".into()])),
		"users_to_notify should be 1 'john'"
	);

	let c_note1 = DBChunk::from(format!("# Note 1 -> {}\n", &id_notes).as_str());
	let _id_note1 = c_note1.chunk().id.clone();
	assert!(db.set_chunk(c_note1, "john").is_ok());

	let _all: Vec<ChunkView> = db
		.get_chunks("john")
		.into_iter()
		.map(|v| ChunkView::from((v, "john")))
		.collect();

	let subtree = db.subtree(None, &"john".into(), &|v| v, &|v| json!(ChunkId::from(v)), 2);
	// println!("{subtree:?}");
	assert_eq!(
		subtree.1.unwrap().len(),
		1,
		"Children should be 1 as john has 1 chunk without parents"
	);

	let subtree = db.subtree(
		db.get_chunk(id_notes.as_str(), "john").as_ref(),
		&"john".into(),
		&|v| v,
		&|v| json!(ChunkId::from(v)),
		2,
	);
	// println!("{subtree:?}");
	assert_eq!(
		subtree.1.unwrap().len(),
		1,
		"Children should be 1 as x has 1 chunk without parents"
	);
}
#[test]
fn circular() {
	let mut db = DB::default();

	let c_notes: DBChunk = "# Notes\n".into();
	let id_notes = c_notes.chunk().id.clone();
	// Add '# Notes\n' john
	assert!(db.set_chunk(c_notes, "john").is_ok());

	let c_note1 = DBChunk::from(format!("# Note 1 -> {}\n", &id_notes).as_str());
	let id_note1 = c_note1.chunk().id.clone();
	assert!(db.set_chunk(c_note1, "john").is_ok());

	assert!(
		db.set_chunk((&*id_notes, &*format!("# Notes -> {}\n", &id_notes)).into(), "john")
			.is_err(),
		"Chunk links to itself, A -> A, it should fail."
	);
	assert!(
		db.set_chunk((&*id_notes, &*format!("# Notes -> {}\n", &id_note1)).into(), "john")
			.is_err(),
		"Chunk links circurlarly, A -> B -> A, it should fail."
	);

	let c_note2 = DBChunk::from(format!("# Note 2 -> {}\n", &id_note1).as_str());
	let id_note2 = c_note2.chunk().id.clone();
	assert!(db.set_chunk(c_note2, "john").is_ok());

	assert!(
		db.set_chunk((&*id_notes, &*format!("# Notes -> {}\n", &id_note2)).into(), "sara")
			.is_err(),
		"Chunk links circurlarly, A -> C -> B -> A, it should fail."
	);
}

fn init() -> DB {
	let mut db = DB::default();
	let chunk: DBChunk = ("# Todo \n").into();
	let _id = chunk.chunk().id.clone();
	assert!(db.set_chunk(chunk, "nina").is_ok());
	db
}

#[test]
fn linking() {
	let mut db = init();

	{
		let _all = db.chunks.values().map(|v| v.read().unwrap()).collect::<Vec<_>>();
		// println!("{all:?}");
	}
	db.link_all().unwrap();
	{
		let _all = db.chunks.values().map(|v| v.read().unwrap()).collect::<Vec<_>>();
		// println!("{all:?}");
	}
}
