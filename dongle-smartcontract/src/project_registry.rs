use crate::errors::ContractError;
use crate::storage_keys::StorageKey;
use crate::types::{Project, ProjectRegistrationParams, ProjectUpdateParams, VerificationStatus};
use crate::validation;
use soroban_sdk::{Address, Env, Vec};

pub struct ProjectRegistry;

impl ProjectRegistry {
    #[allow(clippy::too_many_arguments)]
    pub fn register_project(
        env: &Env,
        params: ProjectRegistrationParams,
    ) -> Result<u64, ContractError> {
        params.owner.require_auth();

        // Validate all inputs
        validation::validate_project_name(&params.name)?;
        validation::validate_description(&params.description)?;
        validation::validate_category(&params.category)?;
        validation::validate_website(&params.website)?;
        validation::validate_cid(&params.logo_cid)?;
        validation::validate_cid(&params.metadata_cid)?;

        // Check if project name already exists
        if env
            .storage()
            .persistent()
            .has(&StorageKey::ProjectByName(params.name.clone()))
        {
            return Err(ContractError::ProjectAlreadyExists);
        }

        let mut count: u64 = env
            .storage()
            .persistent()
            .get(&StorageKey::ProjectCount)
            .unwrap_or(0);
        count = count.saturating_add(1);

        let now = env.ledger().timestamp();
        let project = Project {
            id: count,
            owner: params.owner.clone(),
            name: params.name.clone(),
            description: params.description,
            category: params.category,
            website: params.website,
            logo_cid: params.logo_cid,
            metadata_cid: params.metadata_cid,
            verification_status: VerificationStatus::Unverified,
            created_at: now,
            updated_at: now,
        };

        env.storage()
            .persistent()
            .set(&StorageKey::Project(count), &project);
        env.storage()
            .persistent()
            .set(&StorageKey::ProjectCount, &count);
        env.storage()
            .persistent()
            .set(&StorageKey::ProjectByName(params.name), &count);

        let mut owner_projects: Vec<u64> = env
            .storage()
            .persistent()
            .get(&StorageKey::OwnerProjects(params.owner.clone()))
            .unwrap_or_else(|| Vec::new(env));
        owner_projects.push_back(count);
        env.storage()
            .persistent()
            .set(&StorageKey::OwnerProjects(params.owner), &owner_projects);

        Ok(count)
    }

    pub fn update_project(env: &Env, params: ProjectUpdateParams) -> Option<Project> {
        let mut project = Self::get_project(env, params.project_id)?;

        params.caller.require_auth();
        if project.owner != params.caller {
            return None;
        }

        // Validate updated fields
        if let Some(ref value) = params.name {
            validation::validate_project_name(value).ok()?;
            project.name = value.clone();
        }
        if let Some(ref value) = params.description {
            validation::validate_description(value).ok()?;
            project.description = value.clone();
        }
        if let Some(ref value) = params.category {
            validation::validate_category(value).ok()?;
            project.category = value.clone();
        }
        if let Some(ref value) = params.website {
            validation::validate_website(value).ok()?;
            project.website = value.clone();
        }
        if let Some(ref value) = params.logo_cid {
            validation::validate_cid(value).ok()?;
            project.logo_cid = value.clone();
        }
        if let Some(ref value) = params.metadata_cid {
            validation::validate_cid(value).ok()?;
            project.metadata_cid = value.clone();
        }

        project.updated_at = env.ledger().timestamp();
        env.storage()
            .persistent()
            .set(&StorageKey::Project(params.project_id), &project);

        Some(project)
    }

    pub fn get_project(env: &Env, project_id: u64) -> Option<Project> {
        env.storage()
            .persistent()
            .get(&StorageKey::Project(project_id))
    }

    pub fn get_projects_by_owner(env: &Env, owner: Address) -> Vec<Project> {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&StorageKey::OwnerProjects(owner))
            .unwrap_or_else(|| Vec::new(env));

        let mut projects = Vec::new(env);
        let len = ids.len();
        for i in 0..len {
            if let Some(project_id) = ids.get(i) {
                if let Some(project) = Self::get_project(env, project_id) {
                    projects.push_back(project);
                }
            }
        }

        projects
    }

    fn owner_project_count(env: &Env, owner: &Address) -> u32 {
        env.storage()
            .persistent()
            .get(&StorageKey::OwnerProjects(owner.clone()))
            .unwrap_or_else(|| Vec::<u64>::new(env))
            .len()
    }

    pub fn get_owner_project_count(env: &Env, owner: &Address) -> u32 {
        Self::owner_project_count(env, owner)
    }

    pub fn list_projects(env: &Env, start_id: u64, limit: u32) -> Vec<Project> {
        // Validate pagination parameters
        if validation::validate_pagination(limit).is_err() {
            return Vec::new(env);
        }

        let count: u64 = env
            .storage()
            .persistent()
            .get(&StorageKey::ProjectCount)
            .unwrap_or(0);

        let mut projects = Vec::new(env);
        if start_id == 0 || start_id > count {
            return projects;
        }
        let end = core::cmp::min(
            start_id.saturating_add(limit as u64),
            count.saturating_add(1),
        );
        for id in start_id..end {
            if let Some(project) = Self::get_project(env, id) {
                projects.push_back(project);
            }
        }
        projects
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // Tests moved to validation module
}
