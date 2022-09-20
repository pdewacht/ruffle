use crate::mesh::Mesh;
use crate::RegistryData;
use fnv::FnvHashMap;
use ruffle_render::bitmap::BitmapHandle;

pub struct Library {
    pub bitmaps: FnvHashMap<BitmapHandle, RegistryData>,
    pub meshes: Vec<Mesh>,
    next_bitmap_handle: BitmapHandle,
}

impl Library {
    pub fn new() -> Self {
        Self {
            bitmaps: Default::default(),
            meshes: vec![],
            next_bitmap_handle: BitmapHandle(0),
        }
    }

    pub fn next_bitmap_handle(&mut self) -> BitmapHandle {
        let handle = self.next_bitmap_handle;
        self.next_bitmap_handle = BitmapHandle(self.next_bitmap_handle.0 + 1);
        handle
    }
}
