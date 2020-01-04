use failure::Fail;

use crate::component::Component;

#[derive(Clone, Debug)]
pub struct Resource {
    val: i64,
    min: Option<i64>,
    max: Option<i64>,
}

impl Resource {
    pub fn new(val: i64, min: Option<i64>, max: Option<i64>) -> Resource {
        Resource { val, min, max }
    }

    pub fn val(&self) -> i64 {
        self.val
    }

    pub fn min(&self) -> Option<i64> {
        self.min
    }

    pub fn max(&self) -> Option<i64> {
        self.max
    }

    pub fn checked_set(&mut self, rhs: Self) -> Result<(), InvalidSet> {
        let v = rhs.val;
        if let Some(max) = self.max {
            if v > max {
                return Err(InvalidSet(v, self.min, self.max));
            }
        }

        if let Some(min) = self.min {
            if v < min {
                return Err(InvalidSet(v, self.min, self.max));
            }
        }

        self.val = v;
        Ok(())
    }

    pub fn capped_set(&mut self, rhs: Self) {
        let mut v = rhs.val;
        if let Some(min) = self.min {
            if v < min {
                v = min;
            }
        }

        if let Some(max) = self.max {
            if v > max {
                v = max;
            }
        }

        self.val = v;
    }

    pub fn checked_add(&mut self, rhs: Self) -> Result<(), InvalidAddition> {
        let v = self.val + rhs.val;
        if let Some(max) = self.max {
            if v > max {
                return Err(InvalidAddition(rhs.val, self.val, max));
            }
        }
        self.val = v;
        Ok(())
    }

    pub fn capped_add(&mut self, rhs: Self) {
        let v = self.val + rhs.val;
        self.val = self.max.map_or(v, |max| if v > max { max } else { v });
    }

    pub fn checked_sub(&mut self, rhs: Self) -> Result<(), InvalidSubtraction> {
        let v = self.val - rhs.val;
        if let Some(min) = self.min {
            if v < min {
                return Err(InvalidSubtraction(rhs.val, self.val, min));
            }
        }
        self.val = v;
        Ok(())
    }

    pub fn capped_sub(&mut self, rhs: Self) {
        let v = self.val - rhs.val;
        self.val = self.min.map_or(v, |min| if v < min { min } else { v });
    }

    pub fn soft_set_min(&mut self, new_min: Option<i64>) -> Result<(), InvalidSoftCapAdjustment> {
        if let Some(min) = new_min {
            if self.val < min {
                return Err(InvalidSoftCapAdjustment(self.val, min));
            }
        }

        self.min = new_min;
        Ok(())
    }

    pub fn hard_set_min(&mut self, new_min: Option<i64>) {
        if let Some(min) = new_min {
            if self.val < min {
                self.val = min;
            }
        }

        self.min = new_min;
    }

    pub fn soft_set_max(&mut self, new_max: Option<i64>) -> Result<(), InvalidSoftCapAdjustment> {
        if let Some(max) = new_max {
            if self.val > max {
                return Err(InvalidSoftCapAdjustment(self.val, max));
            }
        }

        self.max = new_max;
        Ok(())
    }

    pub fn hard_set_max(&mut self, new_max: Option<i64>) {
        if let Some(max) = new_max {
            if self.val > max {
                self.val = max;
            }
        }

        self.max = new_max;
    }
}

impl From<i64> for Resource {
    fn from(val: i64) -> Resource {
        Resource::new(val, None, None)
    }
}

impl From<Resource> for i64 {
    fn from(rsc: Resource) -> i64 {
        rsc.val
    }
}

impl Component for Resource {}

#[derive(Fail, Debug)]
#[fail(
    display = "Not enough resource (attempted to subtract {} from {}, min is {})",
    _0, _1, _2
)]
pub struct InvalidSubtraction(i64, i64, i64);

#[derive(Fail, Debug)]
#[fail(
    display = "Too much resource (attempted to add {} to {}, cap is {})",
    _0, _1, _2
)]
pub struct InvalidAddition(i64, i64, i64);

#[derive(Fail, Debug)]
#[fail(
    display = "Invalid resource set operation (value of {} is outside of range {:?} to {:?})",
    _0, _1, _2
)]
pub struct InvalidSet(i64, Option<i64>, Option<i64>);

#[derive(Fail, Debug)]
#[fail(
    display = "Invalid soft cap adjustment (current value of {} lies beyond {})",
    _0, _1
)]
pub struct InvalidSoftCapAdjustment(i64, i64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checked_set() {
        let mut rsc = Resource::new(50, Some(0), Some(100));

        // Sets within the stored bounds should be OK.
        assert!(rsc.checked_set(25.into()).is_ok());
        assert_eq!(rsc.val(), 25);

        // Sets outside the bounds should error.
        assert!(rsc.checked_set((-20).into()).is_err());
        assert!(rsc.checked_set(111.into()).is_err());
        assert_eq!(rsc.val(), 25);
    }

    #[test]
    fn test_capped_set() {
        let mut rsc = Resource::new(50, Some(0), Some(100));

        // Sets within the stored bounds work as usual.
        rsc.capped_set(25.into());
        assert_eq!(rsc.val(), 25);

        // Sets outside the bounds cap at the min and max values,
        // respectively.
        rsc.capped_set((-20).into());
        assert_eq!(rsc.val(), 0);

        rsc.capped_set(111.into());
        assert_eq!(rsc.val(), 100);
    }

    #[test]
    fn test_checked_add() {
        let mut rsc = Resource::new(50, Some(0), Some(100));

        // Adds within the stored upper bound should be OK.
        assert!(rsc.checked_add(25.into()).is_ok());
        assert_eq!(rsc.val(), 75);

        // Adds that would go past the bound should error.
        assert!(rsc.checked_add(50.into()).is_err());
        assert_eq!(rsc.val(), 75);
    }

    #[test]
    fn test_capped_add() {
        let mut rsc = Resource::new(50, Some(0), Some(100));

        // Adds within the stored bound act normally.
        rsc.capped_add(25.into());
        assert_eq!(rsc.val(), 75);

        // Adds that would go beyond cap at the max value.
        rsc.capped_add(50.into());
        assert_eq!(rsc.val(), 100);
    }

    #[test]
    fn test_checked_sub() {
        let mut rsc = Resource::new(33, Some(0), Some(100));

        // Subtractions within the stored lower bound are OK.
        assert!(rsc.checked_sub(22.into()).is_ok());
        assert_eq!(rsc.val(), 11);

        // Subtractions outside the lower bound error.
        assert!(rsc.checked_sub(22.into()).is_err());
        assert_eq!(rsc.val(), 11)
    }

    #[test]
    fn test_capped_sub() {
        let mut rsc = Resource::new(33, Some(0), Some(100));

        // Subtractions within the lower bound act normally.
        rsc.capped_sub(22.into());
        assert_eq!(rsc.val(), 11);

        // Subtractions beyond the lower bound are capped at the lower value.
        rsc.capped_sub(22.into());
        assert_eq!(rsc.val(), 0);
    }

    #[test]
    fn test_soft_set_min() {
        let mut rsc = Resource::new(-10, None, None);

        // Attempting to set the min-cap to be higher than the current value errors.
        assert!(rsc.soft_set_min(Some(0)).is_err());
        assert_eq!(rsc.val(), -10);

        // Setting a min-cap less than or equal to the current value is OK.
        assert!(rsc.soft_set_min(Some(-20)).is_ok());
        assert_eq!(rsc.val(), -10);

        // Test to make sure the cap actually got applied.
        assert!(rsc.checked_sub(20.into()).is_err());
        assert_eq!(rsc.val(), -10);

        // Passing None disables the min-cap, and should always work.
        assert!(rsc.soft_set_min(None).is_ok());
        assert!(rsc.checked_sub(20.into()).is_ok());
        assert_eq!(rsc.val(), -30);
    }

    #[test]
    fn test_soft_set_max() {
        let mut rsc = Resource::new(30, None, None);

        // Attempting to set a max-cap lower than the current value errors.
        assert!(rsc.soft_set_max(Some(20)).is_err());
        assert_eq!(rsc.val(), 30);

        // Setting a max-cap greater than or equal to the current value is OK.
        assert!(rsc.soft_set_max(Some(40)).is_ok());
        assert_eq!(rsc.val(), 30);

        // Test to make sure the max-cap works properly.
        assert!(rsc.checked_add(20.into()).is_err());
        assert_eq!(rsc.val(), 30);

        // Passing None disables the max-cap. Again, this always works.
        assert!(rsc.soft_set_max(None).is_ok());
        assert!(rsc.checked_add(20.into()).is_ok());
        assert_eq!(rsc.val(), 50);
    }

    #[test]
    fn test_hard_set_min() {
        let mut rsc = Resource::new(-10, None, None);

        // Setting a min-cap less than or equal to the current value doesn't affect the value.
        rsc.hard_set_min(Some(-20));
        assert_eq!(rsc.val(), -10);
        assert!(rsc.checked_sub(20.into()).is_err());

        // Setting a min-cap greater than the current value, though, sets the value to the new min-cap.
        rsc.hard_set_min(Some(0));
        assert_eq!(rsc.val(), 0);
        assert!(rsc.checked_sub(1.into()).is_err());

        // As expected, passing None disables the min-cap without otherwise affecting the value.
        rsc.hard_set_min(None);
        assert!(rsc.checked_sub(10.into()).is_ok());
        assert_eq!(rsc.val(), -10);
    }

    #[test]
    fn test_hard_set_max() {
        let mut rsc = Resource::new(30, None, None);

        // Setting a max-cap greater than or equal to the current value doesn't affect the value.
        rsc.hard_set_max(Some(40));
        assert_eq!(rsc.val(), 30);
        assert!(rsc.checked_add(20.into()).is_err());

        // Setting a max-cap less than the current value sets the value to the new max-cap.
        rsc.hard_set_max(Some(0));
        assert_eq!(rsc.val(), 0);
        assert!(rsc.checked_add(1.into()).is_err());

        // Again, passing None disables the max-cap without touching the current value.
        rsc.hard_set_max(None);
        assert!(rsc.checked_add(20.into()).is_ok());
        assert_eq!(rsc.val(), 20);
    }
}
