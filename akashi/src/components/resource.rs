//! A data type for player-held game resource counts.

use failure::Fail;

use crate::ecs::Component;
use crate::player::Player;

/// Represents an arbitrary numeric player resource, with optional
/// lower and upper caps.
#[derive(Clone, Debug)]
pub struct Resource {
    val: i64,
    min: Option<i64>,
    max: Option<i64>,
}

impl Resource {
    /// Creates a new `Resource` instance representing the given value,
    /// with optional lower and upper caps.
    pub fn new(val: i64, min: Option<i64>, max: Option<i64>) -> Resource {
        Resource { val, min, max }
    }

    /// Gets the current value represented by this `Resource`.
    pub fn val(&self) -> i64 {
        self.val
    }

    /// Gets this `Resource`'s lower cap, if any.
    pub fn min(&self) -> Option<i64> {
        self.min
    }

    /// Gets this `Resource`'s upper cap, if any.
    pub fn max(&self) -> Option<i64> {
        self.max
    }

    /// Sets this resource's value to the value of the passed-in
    /// `Resource` instance.
    ///
    /// # Errors
    ///
    /// Returns an `InvalidSet` error if the new value would be
    /// outside this `Resource`'s configured bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(0, Some(0), Some(100));
    ///
    /// // 0 < 50 < 100: ok
    /// let result = rsc.checked_set(50.into());
    /// assert!(result.is_ok());
    /// assert_eq!(rsc.val(), 50);
    ///
    /// // 150 > 100: error
    /// let result = rsc.checked_set(150.into());
    /// assert!(result.is_err());
    /// assert_eq!(rsc.val(), 50);
    /// ```
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

    /// Sets this resource's value to the value of the passed-in
    /// `Resource` instance, capping the new value so that it fits
    /// within this `Resource`'s configured bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(50, Some(0), Some(100));
    ///
    /// // -50 < 0: value capped to 0
    /// rsc.capped_set((-50).into());
    /// assert_eq!(rsc.val(), 0);
    ///
    /// // 150 > 100: value capped to 100
    /// rsc.capped_set(150.into());
    /// assert_eq!(rsc.val(), 100);
    /// ```
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

    /// Adds the value contained in another `Resource` to this one.
    ///
    /// # Errors
    ///
    /// Returns an `InvalidAddition` error if the new value would be
    /// outside this `Resource`'s configured bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(0, Some(0), Some(100));
    ///
    /// // 0 + 50 = 50, and 0 < 50 < 100, so this is okay.
    /// let result = rsc.checked_add(50.into());
    /// assert!(result.is_ok());
    /// assert_eq!(rsc.val(), 50);
    ///
    /// // 50 + 75 = 125, but 125 > 100, so this errors.
    /// let result = rsc.checked_add(75.into());
    /// assert!(result.is_err());
    /// assert_eq!(rsc.val(), 50);
    /// ```
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

    /// Adds the value contained in another `Resource` to this one,
    /// capping the new value at the configured upper bound if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(50, Some(0), Some(100));
    ///
    /// // 50 + 100 = 150.
    /// // Since 150 > 100, the cap is applied.
    /// rsc.capped_add(100.into());
    /// assert_eq!(rsc.val(), 100);
    /// ```
    pub fn capped_add(&mut self, rhs: Self) {
        let v = self.val + rhs.val;
        self.val = self.max.map_or(v, |max| if v > max { max } else { v });
    }

    /// Subtracts the value contained in another `Resource` from this one.
    ///
    /// # Errors
    ///
    /// Returns an `InvalidSubtraction` error if the new value would be
    /// outside this `Resource`'s configured bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(100, Some(0), Some(100));
    ///
    /// // 100 - 50 = 50, and 0 < 50 < 100, so this is okay.
    /// let result = rsc.checked_sub(50.into());
    /// assert!(result.is_ok());
    /// assert_eq!(rsc.val(), 50);
    ///
    /// // 50 - 75 = -25, but -25 < 0, so this errors.
    /// let result = rsc.checked_sub(75.into());
    /// assert!(result.is_err());
    /// assert_eq!(rsc.val(), 50);
    /// ```
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

    /// Subtracts the value contained in another `Resource` from this one,
    /// capping the new value at the configured lower bound, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(50, Some(0), Some(100));
    ///
    /// // 50 - 100 = -25.
    /// // Since -25 < 0, the new value is capped to 0.
    /// rsc.capped_sub(100.into());
    /// assert_eq!(rsc.val(), 0);
    /// ```
    pub fn capped_sub(&mut self, rhs: Self) {
        let v = self.val - rhs.val;
        self.val = self.min.map_or(v, |min| if v < min { min } else { v });
    }

    /// Sets the lower cap for this `Resource` to a new value.
    ///
    /// # Errors
    ///
    /// Returns an `InvalidSoftCapAdjustment` error if the currently-stored
    /// value in this `Resource` would be outside the new bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(500, None, None);
    ///
    /// // 500 > 0: ok
    /// let result = rsc.soft_set_min(Some(0));
    /// assert!(result.is_ok());
    ///
    /// // 500 < 1000: error
    /// let result = rsc.soft_set_min(Some(1000));
    /// assert!(result.is_err());
    /// ```
    pub fn soft_set_min(&mut self, new_min: Option<i64>) -> Result<(), InvalidSoftCapAdjustment> {
        if let Some(min) = new_min {
            if self.val < min {
                return Err(InvalidSoftCapAdjustment(self.val, min));
            }
        }

        self.min = new_min;
        Ok(())
    }

    /// Sets the lower cap for this `Resource` to a new value, applying
    /// the new cap to the currently contained value if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(500, None, None);
    ///
    /// // 500 > 0: no change
    /// rsc.hard_set_min(Some(0));
    /// assert_eq!(rsc.val(), 500);
    ///
    /// // 500 < 1000: value set to new lower cap
    /// rsc.hard_set_min(Some(1000));
    /// assert_eq!(rsc.val(), 1000);
    pub fn hard_set_min(&mut self, new_min: Option<i64>) {
        if let Some(min) = new_min {
            if self.val < min {
                self.val = min;
            }
        }

        self.min = new_min;
    }

    /// Sets the upper cap for this `Resource` to a new value.
    ///
    /// # Errors
    ///
    /// Returns an `InvalidSoftCapAdjustment` error if the currently-stored
    /// value in this `Resource` would be outside the new bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(500, None, None);
    ///
    /// // 500 < 1000: ok
    /// let result = rsc.soft_set_max(Some(1000));
    /// assert!(result.is_ok());
    ///
    /// // 500 > 250: error
    /// let result = rsc.soft_set_max(Some(250));
    /// assert!(result.is_err());
    /// ```
    pub fn soft_set_max(&mut self, new_max: Option<i64>) -> Result<(), InvalidSoftCapAdjustment> {
        if let Some(max) = new_max {
            if self.val > max {
                return Err(InvalidSoftCapAdjustment(self.val, max));
            }
        }

        self.max = new_max;
        Ok(())
    }

    /// Sets the upper cap for this `Resource` to a new value, applying
    /// the new cap to the currently contained value if necessary.
    ///
    /// # Examples
    ///
    /// ```
    /// use akashi::components::Resource;
    /// let mut rsc = Resource::new(500, None, None);
    ///
    /// // 500 < 1000: no change
    /// rsc.hard_set_max(Some(1000));
    /// assert_eq!(rsc.val(), 500);
    ///
    /// // 500 > 250: value set to new upper cap
    /// rsc.hard_set_max(Some(250));
    /// assert_eq!(rsc.val(), 250);
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

impl Component<Player> for Resource {}

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
