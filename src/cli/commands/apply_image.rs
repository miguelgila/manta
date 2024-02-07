use std::{collections::HashMap, path::PathBuf};

use mesa::{
    cfs::{
        self,
        configuration::mesa::r#struct::{
            cfs_configuration_request::CfsConfigurationRequest,
            cfs_configuration_response::{ApiError, CfsConfigurationResponse},
        },
        session::mesa::r#struct::{CfsSessionGetResponse, CfsSessionPostRequest},
    },
    common::{kubernetes, vault::http_client::fetch_shasta_k8s_secrets},
};
use serde_yaml::Value;

use crate::common::{cfs_session_utils, jwt_ops::get_claims_from_jwt_token};

/// Creates a CFS configuration and a CFS session from a CSCS SAT file.
/// Note: this method will fail if session name collide. This case happens if the __DATE__
/// placeholder is missing in the session name
/// Return a tuple (<cfs configuration name>, <cfs session name>)
pub async fn exec(
    vault_base_url: &str,
    vault_secret_path: &str,
    vault_role_id: &str,
    path_file: &PathBuf,
    shasta_token: &str,
    shasta_base_url: &str,
    shasta_root_cert: &[u8],
    ansible_verbosity_opt: Option<&String>,
    ansible_passthrough_opt: Option<&String>,
    watch_logs_opt: Option<&bool>,
    tag: &str,
    hsm_group_available_vec: &[String],
    k8s_api_url: &str,
    gitea_token: &str,
    output_opt: Option<&String>,
) -> (Vec<CfsConfigurationResponse>, Vec<CfsSessionGetResponse>) {
    let file_content = std::fs::read_to_string(path_file).expect("SAT file not found. Exit");
    let sat_file_yaml: Value = serde_yaml::from_str(&file_content).unwrap();

    // VALIDATION - WE WON'T PROCESS ANYTHING IF THE USER DOES NOT HAVE ACCESS TO ANY HSM GROUP
    // DEFINED IN THE SAT FILE

    // Get CFS configurations from SAT YAML file
    let configuration_yaml_vec_opt = sat_file_yaml["configurations"].as_sequence();

    // Get inages from SAT YAML file
    let image_yaml_vec_opt = sat_file_yaml["images"].as_sequence();

    // Get inages from SAT YAML file
    let bos_session_template_list_yaml = sat_file_yaml["session_templates"].as_sequence();

    if bos_session_template_list_yaml.is_some() {
        log::warn!(
            "SAT file has data in session_template section. This information will be ignored."
        )
    }

    // Check HSM groups in images section in SAT file matches the HSM group in JWT (keycloak roles)
    validate_sat_file_images_section(image_yaml_vec_opt, hsm_group_available_vec);

    let shasta_k8s_secrets = crate::common::vault::http_client::fetch_shasta_k8s_secrets(
        vault_base_url,
        vault_secret_path,
        vault_role_id,
    )
    .await;

    let kube_client = kubernetes::get_k8s_client_programmatically(k8s_api_url, shasta_k8s_secrets)
        .await
        .unwrap();
    let cray_product_catalog = kubernetes::get_configmap(kube_client, "cray-product-catalog")
        .await
        .unwrap();

    let mut cfs_configuration_hashmap = HashMap::new();

    for configuration_yaml in configuration_yaml_vec_opt.unwrap_or(&Vec::new()) {
        let cfs_configuration_rslt: Result<CfsConfigurationResponse, ApiError> =
            mesa::cfs::configuration::mesa::utils::create_from_sat_file(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                gitea_token,
                &cray_product_catalog,
                configuration_yaml,
                tag,
            )
            .await;

        let cfs_configuration = match cfs_configuration_rslt {
            Ok(cfs_configuration) => cfs_configuration,
            Err(error) => {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        };

        cfs_configuration_hashmap.insert(cfs_configuration.name.clone(), cfs_configuration.clone());
    }

    // Process CFS sessions
    let mut cfs_session_created_hashmap: HashMap<String, CfsSessionGetResponse> = HashMap::new();

    for image_yaml in image_yaml_vec_opt.unwrap_or(&Vec::new()) {
        /* let mut cfs_session = CfsSessionPostRequest::from_sat_file_serde_yaml(image_yaml).unwrap();

        // Rename session name
        cfs_session.name = cfs_session.name.replace("__DATE__", tag);

        // Rename session's configuration name
        cfs_session.configuration_name = cfs_session.configuration_name.replace("__DATE__", tag);

        // Set ansible verbosity
        cfs_session.ansible_verbosity = Some(
            ansible_verbosity_opt
                .cloned()
                .unwrap_or("0".to_string())
                .parse::<u8>()
                .unwrap(),
        );

        // Set ansible passthrough params
        cfs_session.ansible_passthrough = ansible_passthrough_opt.cloned();

        log::debug!("CFS session creation payload:\n{:#?}", cfs_session);

        let create_cfs_session_resp = cfs::session::mesa::http_client::post(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            &cfs_session,
        )
        .await;

        log::debug!(
            "CFS session creation response:\n{:#?}",
            create_cfs_session_resp
        );

        if create_cfs_session_resp.is_err() {
            eprintln!("CFS session creation failed");
            eprintln!("Reason:\n{:#?}", create_cfs_session_resp);
            std::process::exit(1);
        } */

        let cfs_session_rslt = cfs::session::mesa::utils::create_from_sat_file(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            &cray_product_catalog,
            image_yaml,
            ansible_verbosity_opt,
            ansible_passthrough_opt,
            tag,
        )
        .await;

        /* let cfs_session = match cfs_session_rslt {
            Ok(cfs_session) => cfs_session,
            Err(error) => {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        };

        log::info!(
            "CFS session created: {}",
            cfs_session.name.as_ref().unwrap()
        );

        cfs_session_created_hashmap.insert(cfs_session.name.clone().unwrap(), cfs_session.clone());

        // cfs_session_name_list.push(cfs_session.clone());

        // Print output
        if output_opt.is_some() && output_opt.unwrap().eq("json") {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &cfs_session_created_hashmap
                        .values()
                        .cloned()
                        .collect::<Vec<CfsSessionGetResponse>>()
                )
                .unwrap()
            );
        } else {
            cfs_session_utils::print_table_struct(
                &cfs_session_created_hashmap.values().cloned().collect(),
            );
        }

        // Audit to file
        let jwt_claims = get_claims_from_jwt_token(shasta_token).unwrap();

        log::info!(target: "app::audit", "User: {} ({}) ; Operation: Apply image", jwt_claims["name"].as_str().unwrap(), jwt_claims["preferred_username"].as_str().unwrap());

        if let Some(true) = watch_logs_opt {
            log::info!("Fetching logs ...");

            /* let mut logs_stream = cli::commands::log::get_cfs_session_container_ansible_logs_stream(
                vault_base_url,
                vault_secret_path,
                vault_role_id,
                &cfs_session.name,
                None,
                k8s_api_url,
            )
            .await
            .unwrap(); */

            let shasta_k8s_secrets =
                fetch_shasta_k8s_secrets(vault_base_url, vault_secret_path, vault_role_id).await;

            let client =
                kubernetes::get_k8s_client_programmatically(k8s_api_url, shasta_k8s_secrets)
                    .await
                    .unwrap();

            kubernetes::print_cfs_session_logs(client, cfs_session.name.unwrap().as_str()).await;
        } */
    }

    (
        cfs_configuration_hashmap.values().cloned().collect(),
        cfs_session_created_hashmap.values().cloned().collect(),
    )
}

pub fn validate_sat_file_images_section(
    image_yaml_vec_opt: Option<&Vec<Value>>,
    hsm_group_available_vec: &[String],
) {
    // Check HSM groups in images section in SAT file matches the HSM group in JWT (keycloak roles)
    for image_yaml_vec in image_yaml_vec_opt.unwrap_or(&Vec::new()) {
        for hsm_group in image_yaml_vec["configuration_group_names"]
            .as_sequence()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|hsm_group_yaml| hsm_group_yaml.as_str().unwrap())
            .filter(|&hsm_group| {
                !hsm_group.eq_ignore_ascii_case("Compute")
                    && !hsm_group.eq_ignore_ascii_case("Application")
                    && !hsm_group.eq_ignore_ascii_case("Application_UAN")
            })
        {
            if !hsm_group_available_vec.contains(&hsm_group.to_string()) {
                println!(
                        "HSM group '{}' in image {} not allowed, List of HSM groups available {:?}. Exit",
                        hsm_group,
                        image_yaml_vec["name"].as_str().unwrap(),
                        hsm_group_available_vec
                    );
                std::process::exit(-1);
            }
        }
    }
}
