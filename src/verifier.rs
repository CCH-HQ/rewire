use crate::{Recipe, format::merge_recipe};
use anyhow::{Result, anyhow};

/// Re-parse the bytes that reached disk; this catches filesystem/rename corruption before commit.
pub fn verify_recipe(recipe: &Recipe, bytes: &[u8]) -> Result<()> {
    let verified = merge_recipe(recipe, Some(bytes))?;
    if verified != bytes {
        return Err(anyhow!(
            "written configuration does not contain the complete recipe"
        ));
    }
    Ok(())
}
