use failure::Fail;

use crate::component::Component;

pub struct Resource {
    min: Option<u64>,
    max: Option<u64>,
    val: u64,
}

impl Resource {
    pub fn new(val: u64, min: Option<u64>, max: Option<u64>) -> Resource {
        Resource { val, min, max }
    }

    pub fn val(&self) -> u64 { self.val }
    pub fn min(&self) -> Option<u64> { self.min }
    pub fn max(&self) -> Option<u64> { self.max }

    pub fn checked_add(&mut self, rhs: &Self) -> Result<(), InvalidAddition> {
        let v = self.val + rhs.val;
        if let Some(max) = self.max {
            if v > max {
                return Err(InvalidAddition(rhs.val, self.val, max));
            }
        }
        self.val = v;
        Ok(())
    }

    pub fn capped_add(&mut self, rhs: &Self) {
        let v = self.val + rhs.val;
        self.val = self.max.map_or(v, |max| if v > max { max } else { v });
    }

    pub fn checked_sub(&mut self, rhs: &Self) -> Result<(), InvalidSubtraction> {
        let v = self.val - rhs.val;
        if let Some(min) = self.min {
            if v < min {
                return Err(InvalidSubtraction(rhs.val, self.val, min));
            }
        }
        self.val = v;
        Ok(())
    }

    pub fn capped_sub(&mut self, rhs: &Self) {
        let v = self.val - rhs.val;
        self.val = self.min.map_or(v, |min| if v < min { min } else { v });
    }

    pub fn soft_set_min(&mut self, new_min: Option<u64>) -> Result<(), InvalidSoftCapAdjustment>{
        if let Some(min) = new_min {
            if self.val < min {
                return Err(InvalidSoftCapAdjustment(self.val, min));
            }
        }

        self.min = new_min;
        Ok(())
    }

    pub fn hard_set_min(&mut self, new_min: Option<u64>) {
        if let Some(min) = new_min {
            if self.val < min {
                self.val = min;
            }
        }
        
        self.min = new_min;
    }

    pub fn soft_set_max(&mut self, new_max: Option<u64>) -> Result<(), InvalidSoftCapAdjustment>{
        if let Some(max) = new_max  {
            if self.val > max {
                return Err(InvalidSoftCapAdjustment(self.val, max));
            }
        }

        self.max = new_max;
        Ok(())
    }

    pub fn hard_set_mmax(&mut self, new_max: Option<u64>) {
        if let Some(max) = new_max  {
            if self.val < max {
                self.val = max;
            }
        }
        
        self.max = new_max;
    }
}

impl Component for Resource {}

#[derive(Fail, Debug)]
#[fail(display = "Not enough resource (attempted to subtract {} from {}, min is {})", _0, _1, _2)]
pub struct InvalidSubtraction(u64, u64, u64);

#[derive(Fail, Debug)]
#[fail(display = "Too much resource (attempted to add {} to {}, cap is {})", _0, _1, _2)]
pub struct InvalidAddition(u64, u64, u64);

#[derive(Fail, Debug)]
#[fail(display = "Invalid soft cap adjustment (current value of {} lies beyond {})", _0, _1)]
pub struct InvalidSoftCapAdjustment(u64, u64);
