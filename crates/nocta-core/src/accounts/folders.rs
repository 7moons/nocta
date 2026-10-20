//! Folders: listing, create/edit/delete and ordering within groups.
use super::*;

#[derive(FromRow)]
struct FolderRow {
    id: i64,
    name: String,
    color: String,
    group_name: Option<String>,
    sort_order: i64,
}

impl Accounts {
    pub async fn folders(&self) -> CoreResult<Vec<Folder>> {
        let rows = sqlx::query_as::<_, FolderRow>("SELECT id, name, color, group_name, sort_order FROM folders ORDER BY COALESCE(group_name, ''), sort_order, id")
            .fetch_all(&self.storage.read).await?;
        Ok(rows
            .into_iter()
            .map(|row| Folder {
                id: row.id as u32,
                name: row.name,
                color: row.color,
                group: row.group_name,
                sort_order: row.sort_order as u32,
            })
            .collect())
    }

    pub async fn save_folder(&self, id: Option<u32>, input: FolderInput) -> CoreResult<()> {
        let name = input.name.trim();
        validate_folder(name, &input.color, input.group.as_deref())?;
        let group = input
            .group
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let result = if let Some(id) = id {
            sqlx::query("UPDATE folders SET name = ?, normalized_name = ?, color = ?, group_name = ? WHERE id = ?")
                .bind(name).bind(name.to_lowercase()).bind(input.color.to_ascii_lowercase()).bind(group).bind(id).execute(&self.storage.pool).await
        } else {
            sqlx::query("INSERT INTO folders (name, normalized_name, color, group_name, sort_order) VALUES (?, ?, ?, ?, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM folders))")
                .bind(name).bind(name.to_lowercase()).bind(input.color.to_ascii_lowercase()).bind(group).execute(&self.storage.pool).await
        };
        map_constraint(result)?;
        Ok(())
    }

    pub async fn remove_folder(&self, id: u32) -> CoreResult<()> {
        sqlx::query("DELETE FROM folders WHERE id = ?")
            .bind(id)
            .execute(&self.storage.pool)
            .await?;
        Ok(())
    }

    pub async fn move_folder(
        &self,
        id: u32,
        group: Option<String>,
        before: Option<u32>,
    ) -> CoreResult<()> {
        let group = group
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        validate_folder("folder", "#000000", group.as_deref())?;
        let mut tx = self.storage.pool.begin().await?;
        map_constraint(
            sqlx::query("UPDATE folders SET group_name = ? WHERE id = ?")
                .bind(&group)
                .bind(id)
                .execute(&mut *tx)
                .await,
        )?;
        let mut ordered: Vec<i64> = sqlx::query_scalar("SELECT id FROM folders WHERE COALESCE(group_name, '') = COALESCE(?, '') AND id != ? ORDER BY sort_order, id")
            .bind(&group).bind(id).fetch_all(&mut *tx).await?;
        let index = match before {
            Some(before) if before != id => ordered
                .iter()
                .position(|candidate| *candidate == before as i64)
                .ok_or(CoreError::NotFound)?,
            _ => ordered.len(),
        };
        ordered.insert(index, id as i64);
        for (index, id) in ordered.into_iter().enumerate() {
            sqlx::query("UPDATE folders SET sort_order = ? WHERE id = ?")
                .bind(index as i64)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Renames a group, or with `to` empty ungroups its folders. Moving into an existing group
    /// keeps that group's folders first, then the moved ones in their order.
    pub async fn rename_folder_group(&self, from: &str, to: Option<String>) -> CoreResult<()> {
        let to = to
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        validate_folder("folder", "#000000", to.as_deref())?;
        let mut tx = self.storage.pool.begin().await?;
        let moved: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM folders WHERE group_name = ? ORDER BY sort_order, id",
        )
        .bind(from)
        .fetch_all(&mut *tx)
        .await?;
        if moved.is_empty() {
            return Err(CoreError::NotFound);
        }
        let mut ordered: Vec<i64> = sqlx::query_scalar("SELECT id FROM folders WHERE COALESCE(group_name, '') = COALESCE(?, '') AND COALESCE(group_name, '') != ? ORDER BY sort_order, id")
            .bind(&to).bind(from).fetch_all(&mut *tx).await?;
        ordered.extend(moved);
        for (index, id) in ordered.into_iter().enumerate() {
            sqlx::query("UPDATE folders SET group_name = ?, sort_order = ? WHERE id = ?")
                .bind(&to)
                .bind(index as i64)
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

pub(super) fn validate_folder(name: &str, color: &str, group: Option<&str>) -> CoreResult<()> {
    if name.trim().is_empty()
        || name.len() > 80
        || name.chars().any(char::is_control)
        || group.is_some_and(|group| group.len() > 80 || group.chars().any(char::is_control))
    {
        return Err(CoreError::Validation(
            "Folder name and group must contain at most 80 printable characters.".into(),
        ));
    }
    if color.len() != 7
        || !color.starts_with('#')
        || !color[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err(CoreError::Validation("Use a six-digit hex color.".into()));
    }
    Ok(())
}
