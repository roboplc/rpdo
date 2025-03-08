use std::{io::Cursor, sync::Arc};

use crate::error::Error;
use crate::{Mutex, Result};
use binrw::{BinRead, BinWrite};

#[allow(clippy::module_name_repetitions)]
pub trait RpdoContext {
    fn get_bytes(&self, register: u32, offset: u32, data_size: u32) -> Result<Vec<u8>>;
    fn set_bytes(&self, register: u32, offset: u32, data: &[u8]) -> Result<()>;
}

#[derive(Clone)]
pub struct Basic {
    data: Arc<Vec<Mutex<Vec<u8>>>>,
    register_flexible: bool,
}

impl Basic {
    pub fn new(register_count: usize, register_size: usize, register_flexible: bool) -> Self {
        Self {
            data: Arc::new(
                (0..register_count)
                    .map(|_| Mutex::new(vec![0; register_size]))
                    .collect(),
            ),
            register_flexible,
        }
    }
    pub fn get<T>(&self, register: u32, offset: u32, data_size: u32) -> Result<T>
    where
        T: for<'a> BinRead<Args<'a> = ()>,
    {
        let mut c = Cursor::new(self.get_bytes(register, offset, data_size)?);
        T::read_le(&mut c).map_err(Into::into)
    }
    pub fn set<T>(&self, register: u32, offset: u32, data: &T) -> Result<()>
    where
        T: for<'a> BinWrite<Args<'a> = ()>,
    {
        let mut c = Cursor::new(Vec::new());
        data.write_le(&mut c)?;
        self.set_bytes(register, offset, &c.into_inner())
    }
}

impl RpdoContext for Basic {
    fn set_bytes(&self, register: u32, offset: u32, data: &[u8]) -> Result<()> {
        let register = usize::try_from(register).unwrap();
        let Some(reg_data) = self.data.get(register) else {
            return Err(Error::InvalidRegister);
        };
        let mut reg_data = reg_data.lock();
        let offset = usize::try_from(offset).unwrap();
        if reg_data.len() < offset + data.len() {
            if !self.register_flexible {
                return Err(Error::InvalidOffset);
            }
            reg_data.resize(offset + data.len(), 0);
        }
        reg_data[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
    fn get_bytes(&self, register: u32, offset: u32, data_size: u32) -> Result<Vec<u8>> {
        let register = usize::try_from(register).unwrap();
        let Some(reg_data) = self.data.get(register) else {
            return Err(Error::InvalidRegister);
        };
        let reg_data = reg_data.lock();
        let offset = usize::try_from(offset).unwrap();
        let mut data_size = usize::try_from(data_size).unwrap();
        if data_size == 0 {
            data_size = reg_data.len() - offset;
        }
        if offset > reg_data.len() {
            if !self.register_flexible {
                return Err(Error::InvalidOffset);
            }
            return Ok(vec![0; data_size]);
        }
        let mut result = reg_data[offset..reg_data.len().min(offset + data_size)].to_vec();
        drop(reg_data);
        if result.len() < data_size {
            if !self.register_flexible {
                return Err(Error::InvalidOffset);
            }
            result.resize(data_size, 0);
        }
        Ok(result)
    }
}
