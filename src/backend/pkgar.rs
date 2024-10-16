use cosmic::widget;
use std::{collections::HashMap, error::Error, fmt::Write, sync::Arc};

use super::{Backend, Package};
use crate::{AppId, AppInfo, AppUrl, AppstreamCache, Operation, OperationKind};

#[derive(Debug)]
pub struct Pkgar {
    appstream_caches: Vec<AppstreamCache>,
}

impl Pkgar {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        let source_id = "pkgar";
        //TODO: translate?
        let source_name = "System";
        Ok(Self {
            appstream_caches: vec![AppstreamCache::system(
                source_id.to_string(),
                source_name.to_string(),
                locale,
            )],
        })
    }
}

impl Backend for Pkgar {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>> {
        for appstream_cache in self.appstream_caches.iter_mut() {
            appstream_cache.reload();
        }
        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        Ok(Vec::new())
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        Ok(Vec::new())
    }

    fn file_packages(&self, path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("Pkgar::file_packages not implemented".into())
    }

    fn operation(
        &self,
        op: &Operation,
        mut f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        Err("Pkgar::operation not implemented".into())
    }
}
