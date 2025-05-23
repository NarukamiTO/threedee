#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

pub mod chunks;

use std::fmt::Debug;
use std::io::{BufRead, Cursor, Seek, SeekFrom};

use byteorder::{LittleEndian, ReadBytesExt};
use tracing::{debug, info};

use crate::chunks::{
  EDIT_MATERIAL, EDIT_VERSION, MAIN3DS, MAIN_EDITOR, MAIN_KEYFRAMES, MAIN_VERSION, MATERIAL_NAME, MATERIAL_TEXTURE_MAP,
  MATERIAL_TEXTURE_MAP_NAME,
};

pub struct Parser3DS<'a> {
  data: &'a mut Cursor<&'a [u8]>,
}

pub struct ChunkInfo {
  id: u16,
  offset: u64,
  next_chunk_offset: u32,
}

impl ChunkInfo {
  #[must_use]
  pub const fn get_end(&self) -> u64 {
    self.offset + self.next_chunk_offset as u64
  }
}

impl Debug for ChunkInfo {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "ChunkInfo(id: 0x{:04x}, offset: 0x{:x}, next_chunk_offset: 0x{:x})",
      self.id, self.offset, self.next_chunk_offset
    ))
  }
}

pub trait CursorExt {
  fn remaining(&self) -> u64;
}

impl<T: AsRef<[u8]>> CursorExt for Cursor<T> {
  fn remaining(&self) -> u64 {
    self.get_ref().as_ref().len() as u64 - self.position()
  }
}

#[derive(Debug)]
pub enum Main {
  Editor(Vec<Editor>),
}

#[derive(Debug)]
pub enum Editor {
  Material(Vec<Material>),
}

#[derive(Debug)]
pub enum Material {
  Name(String),
  TextureMap(Vec<MaterialTextureMap>),
}

#[derive(Debug)]
pub enum MaterialTextureMap {
  Name(String),
}

impl<'a> Parser3DS<'a> {
  pub fn new(data: &'a mut Cursor<&'a [u8]>) -> Self {
    Parser3DS { data }
  }

  pub fn read_chunk_info(&mut self) -> ChunkInfo {
    let offset = self.data.position();
    let id = self.data.read_u16::<LittleEndian>().unwrap();
    let next_chunk_offset = self.data.read_u32::<LittleEndian>().unwrap();
    ChunkInfo {
      id,
      offset,
      next_chunk_offset,
    }
  }

  pub fn seek_to_next_chunk(&mut self, info: &ChunkInfo) {
    self
      .data
      .seek(SeekFrom::Start(info.offset + info.next_chunk_offset as u64))
      .unwrap();
  }

  pub fn read_main(&mut self) -> Vec<Main> {
    let mut items = Vec::new();
    let info = self.read_chunk_info();
    debug!("root chunk info: {:?}", info);
    match info.id {
      MAIN3DS => {
        while self.data.position() < info.get_end() {
          let info = self.read_chunk_info();
          debug!("main chunk {:?}", info);
          match info.id {
            MAIN_VERSION => {
              debug!("version: 0x{:x}", info.id);
              self.seek_to_next_chunk(&info);
            }
            MAIN_EDITOR => {
              debug!("scene chunk");
              items.push(Main::Editor(self.read_editor(&info)));
              self.seek_to_next_chunk(&info);
            }
            MAIN_KEYFRAMES => {
              debug!("animation chunk");
              self.seek_to_next_chunk(&info);
            }
            _ => unimplemented!(),
          }
        }
      }
      _ => unimplemented!(),
    }

    items
  }

  pub fn read_editor(&mut self, info: &ChunkInfo) -> Vec<Editor> {
    let mut items = Vec::new();
    while self.data.position() < info.get_end() {
      let info = self.read_chunk_info();
      // debug!("editor chunk info: {:?}", info);

      match info.id {
        EDIT_VERSION => {
          debug!("editor version: {:?}", info);
          self.seek_to_next_chunk(&info);
        }
        EDIT_MATERIAL => {
          debug!("editor material {:?}", info);
          items.push(Editor::Material(self.read_material(&info)));
          self.seek_to_next_chunk(&info);
        }
        _ => {
          debug!("unknown editor chunk {:?}", info);
          self.seek_to_next_chunk(&info);
        }
      };
    }

    items
  }

  pub fn read_material(&mut self, info: &ChunkInfo) -> Vec<Material> {
    let mut items = Vec::new();
    while self.data.position() < info.get_end() {
      let info = self.read_chunk_info();
      // debug!("material chunk info: {:?}", info);

      match info.id {
        MATERIAL_NAME => {
          let mut name = Vec::new();
          self.data.read_until(b'\0', &mut name).unwrap();
          name.pop();
          let name = String::from_utf8(name).unwrap();
          info!("material name: {:?}", name);
          items.push(Material::Name(name));

          self.seek_to_next_chunk(&info);
        }
        MATERIAL_TEXTURE_MAP => {
          info!("material texture map: {:?}", info);
          items.push(Material::TextureMap(self.read_material_texture_map(&info)));
          self.seek_to_next_chunk(&info);
        }
        _ => {
          debug!("unknown material chunk {:?}", info);
          self.seek_to_next_chunk(&info);
        }
      }
    }

    items
  }

  pub fn read_material_texture_map(&mut self, info: &ChunkInfo) -> Vec<MaterialTextureMap> {
    let mut items = Vec::new();
    while self.data.position() < info.get_end() {
      let info = self.read_chunk_info();
      // debug!("material texture map chunk info: {:?}", info);

      match info.id {
        MATERIAL_TEXTURE_MAP_NAME => {
          let mut name = Vec::new();
          self.data.read_until(b'\0', &mut name).unwrap();
          name.pop();
          let name = String::from_utf8(name).unwrap();
          info!("material texture map name: {:?}", name);
          items.push(MaterialTextureMap::Name(name));

          self.seek_to_next_chunk(&info);
        }
        _ => {
          debug!("unknown material texture map chunk {:?}", info);
          self.seek_to_next_chunk(&info);
        }
      }
    }

    items
  }
}

#[cfg(test)]
mod tests {
  use std::fs;

  use super::*;

  #[test_log::test]
  fn it_works() {
    let data = fs::read("test/tower.3ds").unwrap();
    let mut data = Cursor::new(data.as_slice());
    let mut parser = Parser3DS::new(&mut data);
    debug!("{:#?}", parser.read_main());
  }
}
