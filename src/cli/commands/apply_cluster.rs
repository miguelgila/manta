use core::time;
use std::{
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
};

use mesa::{
    cfs::{
        configuration::mesa::r#struct::cfs_configuration_response::{
            ApiError, CfsConfigurationResponse,
        },
        session::mesa::r#struct::CfsSessionGetResponse,
    },
    common::kubernetes,
    ims, {capmc, hsm},
};
use serde_yaml::Value;

use crate::{
    cli::commands::apply_image::validate_sat_file_images_section,
    common::{
        self, jwt_ops::get_claims_from_jwt_token, sat_file::import_images_section_in_sat_file,
    },
};

pub async fn exec(
    shasta_token: &str,
    shasta_base_url: &str,
    shasta_root_cert: &[u8],
    vault_base_url: &str,
    vault_secret_path: &str,
    vault_role_id: &str,
    k8s_api_url: &str,
    path_file: &PathBuf,
    hsm_group_param_opt: Option<&String>,
    hsm_group_available_vec: &Vec<String>,
    ansible_verbosity_opt: Option<u8>,
    ansible_passthrough_opt: Option<&String>,
    gitea_token: &str,
    tag: &str,
    do_not_reboot: bool,
) {
    let file_content = std::fs::read_to_string(path_file).unwrap();
    let sat_file_yaml: Value = serde_yaml::from_str(&file_content).unwrap();

    // Get hardware pattern from SAT YAML file
    let hardware_yaml_value_vec_opt = sat_file_yaml["hardware"].as_sequence();

    // Get CFS configurations from SAT YAML file
    let configuration_yaml_vec = sat_file_yaml["configurations"].as_sequence();

    // Get inages from SAT YAML file
    let image_yaml_vec_opt = sat_file_yaml["images"].as_sequence();

    // Get inages from SAT YAML file
    let bos_session_template_yaml_vec_opt = sat_file_yaml["session_templates"].as_sequence();

    // VALIDATION
    validate_sat_file_images_section(image_yaml_vec_opt, hsm_group_available_vec);
    // Check HSM groups in session_templates in SAT file section matches the ones in JWT token (keycloak roles) in  file
    // This is a bit messy... images section in SAT file valiidation is done inside apply_image::exec but the
    // validation of session_templates section in the SAT file is below
    validate_sat_file_session_template_section(
        bos_session_template_yaml_vec_opt,
        hsm_group_available_vec,
    );

    // Process "hardware" section in SAT file

    log::info!("hardware pattern: {:?}", hardware_yaml_value_vec_opt);

    // Process "configurations" section in SAT file
    //
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

    let mut cfs_configuration_value_vec = Vec::new();

    let mut cfs_configuration_name_vec = Vec::new();

    for configuration_yaml in configuration_yaml_vec.unwrap_or(&vec![]).iter() {
        let cfs_configuration_rslt: Result<CfsConfigurationResponse, ApiError> =
            common::sat_file::create_cfs_configuration_from_sat_file(
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

        let cfs_configuration_name = cfs_configuration.name.to_string();

        cfs_configuration_name_vec.push(cfs_configuration_name.clone());

        cfs_configuration_value_vec.push(cfs_configuration.clone());
    }

    // Process "images" section in SAT file

    // List of image.ref_name already processed
    let mut ref_name_processed_hashmap: HashMap<String, String> = HashMap::new();

    let cfs_session_created_hashmap: HashMap<String, serde_yaml::Value> =
        import_images_section_in_sat_file(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            &mut ref_name_processed_hashmap,
            image_yaml_vec_opt.unwrap_or(&Vec::new()).to_vec(),
            &cray_product_catalog,
            ansible_verbosity_opt,
            ansible_passthrough_opt,
            tag,
        )
        .await;

    /* let cfs_session_complete_vec: Vec<CfsSessionGetResponse> = Vec::new();

    for image_yaml in image_yaml_vec_opt.unwrap_or(&vec![]) {
        let cfs_session_rslt = common::sat_file::create_image_from_sat_file_serde_yaml(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            image_yaml,
            &cray_product_catalog,
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

        wait_cfs_session_to_complete(shasta_token, shasta_base_url, shasta_root_cert, &cfs_session)
            .await;

        if !cfs_session.is_success() {
            eprintln!("CFS session creation failed.\nExit",);
            std::process::exit(1);
        } */
    } */

    println!(); // Don't delete we do need to print an empty line here for the previous waiting CFS
                // session message

    // Process "session_templates" section in SAT file

    process_session_template_section_in_sat_file(
        shasta_token,
        shasta_base_url,
        shasta_root_cert,
        ref_name_processed_hashmap,
        hsm_group_param_opt,
        hsm_group_available_vec,
        sat_file_yaml,
        &tag,
        do_not_reboot,
    )
    .await;
}

pub fn validate_sat_file_session_template_section(
    bos_session_template_list_yaml: Option<&Vec<Value>>,
    hsm_group_available_vec: &Vec<String>,
) {
    for bos_session_template_yaml in bos_session_template_list_yaml.unwrap_or(&vec![]) {
        let bos_session_template_hsm_groups: Vec<String> = if let Some(boot_sets_compute) =
            bos_session_template_yaml["bos_parameters"]["boot_sets"].get("compute")
        {
            boot_sets_compute["node_groups"]
                .as_sequence()
                .unwrap_or(&vec![])
                .iter()
                .map(|node| node.as_str().unwrap().to_string())
                .collect()
        } else if let Some(boot_sets_compute) =
            bos_session_template_yaml["bos_parameters"]["boot_sets"].get("uan")
        {
            boot_sets_compute["node_groups"]
                .as_sequence()
                .unwrap_or(&vec![])
                .iter()
                .map(|node| node.as_str().unwrap().to_string())
                .collect()
        } else {
            println!("No HSM group found in session_templates section in SAT file");
            std::process::exit(1);
        };

        for hsm_group in bos_session_template_hsm_groups {
            if !hsm_group_available_vec.contains(&hsm_group.to_string()) {
                println!(
                        "HSM group '{}' in session_templates {} not allowed, List of HSM groups available {:?}. Exit",
                        hsm_group,
                        bos_session_template_yaml["name"].as_str().unwrap(),
                        hsm_group_available_vec
                    );
                std::process::exit(-1);
            }
        }
    }
}

pub async fn process_session_template_section_in_sat_file(
    shasta_token: &str,
    shasta_base_url: &str,
    shasta_root_cert: &[u8],
    ref_name_processed_hashmap: HashMap<String, String>,
    hsm_group_param_opt: Option<&String>,
    hsm_group_available_vec: &Vec<String>,
    sat_file_yaml: Value,
    tag: &str,
    do_not_reboot: bool,
) {
    let empty_vec = Vec::new();
    let bos_session_template_list_yaml = sat_file_yaml["session_templates"]
        .as_sequence()
        .unwrap_or(&empty_vec);

    for bos_session_template_yaml in bos_session_template_list_yaml {
        let image_details: ims::image::r#struct::Image = if let Some(bos_session_template_image) =
            bos_session_template_yaml.get("image")
        {
            if let Some(bos_session_template_image_ims) = bos_session_template_image.get("ims") {
                if let Some(bos_session_template_image_ims_name) =
                    bos_session_template_image_ims.get("name")
                {
                    let bos_session_template_image_name = bos_session_template_image_ims_name
                        .as_str()
                        .unwrap()
                        .to_string();

                    // Get base image details
                    mesa::ims::image::utils::get_fuzzy(
                        shasta_token,
                        shasta_base_url,
                        shasta_root_cert,
                        hsm_group_available_vec,
                        Some(&bos_session_template_image_name),
                        None,
                    )
                    .await
                    .unwrap()
                    .first()
                    .unwrap()
                    .0
                    .clone()
                } else {
                    eprintln!("ERROR: no 'image.ims.name' section in session_template.\nExit");
                    std::process::exit(1);
                }
            } else if let Some(bos_session_template_image_image_ref) =
                bos_session_template_image.get("image_ref")
            {
                let image_ref = bos_session_template_image_image_ref
                    .as_str()
                    .unwrap()
                    .to_string();

                let image_id = ref_name_processed_hashmap
                    .get(&image_ref)
                    .unwrap()
                    .to_string();

                // Get Image by id
                ims::image::mesa::http_client::get(
                    shasta_token,
                    shasta_base_url,
                    shasta_root_cert,
                    Some(&image_id),
                )
                .await
                .unwrap()
                .first()
                .unwrap()
                .clone()
            } else {
                eprintln!("ERROR: neither 'image.ims' nor 'image.image_ref' sections found in session_template.image.\nExit");
                std::process::exit(1);
            }
        } else {
            eprintln!("ERROR: no 'image' section in session_template.\nExit");
            std::process::exit(1);
        };

        log::info!("Image details: {:#?}", image_details);

        // Get CFS configuration to configure the nodes
        let bos_session_template_configuration_name = bos_session_template_yaml["configuration"]
            .as_str()
            .unwrap()
            .to_string()
            .replace("__DATE__", tag);

        log::info!(
            "Looking for CFS configuration with name: {}",
            bos_session_template_configuration_name
        );

        let cfs_configuration_vec_rslt = mesa::cfs::configuration::mesa::http_client::get(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            Some(&bos_session_template_configuration_name),
        )
        .await;

        /* mesa::cfs::configuration::mesa::utils::filter(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            &mut cfs_configuration_vec,
            None,
            hsm_group_available_vec,
            None,
        )
        .await; */

        if cfs_configuration_vec_rslt.is_err() || cfs_configuration_vec_rslt.unwrap().is_empty() {
            eprintln!(
                "ERROR: BOS session template configuration not found in SAT file image list."
            );
            std::process::exit(1);
        }

        let ims_image_name = image_details.name.to_string();
        let ims_image_etag = image_details.link.as_ref().unwrap().etag.as_ref().unwrap();
        let ims_image_path = &image_details.link.as_ref().unwrap().path;
        let ims_image_type = &image_details.link.as_ref().unwrap().r#type;

        let bos_session_template_name = bos_session_template_yaml["name"]
            .as_str()
            .unwrap_or("")
            .to_string()
            .replace("__DATE__", tag);

        let bos_session_template_hsm_groups: Vec<String> = if let Some(boot_sets_compute) =
            bos_session_template_yaml["bos_parameters"]["boot_sets"].get("compute")
        {
            boot_sets_compute["node_groups"]
                .as_sequence()
                .unwrap_or(&vec![])
                .iter()
                .map(|node| node.as_str().unwrap().to_string())
                .collect()
        } else if let Some(boot_sets_compute) =
            bos_session_template_yaml["bos_parameters"]["boot_sets"].get("uan")
        {
            boot_sets_compute["node_groups"]
                .as_sequence()
                .unwrap_or(&vec![])
                .iter()
                .map(|node| node.as_str().unwrap().to_string())
                .collect()
        } else {
            println!("No HSM group found in session_templates section in SAT file");
            std::process::exit(1);
        };

        // Check HSM groups in YAML file session_templates.bos_parameters.boot_sets.compute.node_groups matches with
        // Check hsm groups in SAT file includes the hsm_group_param
        let hsm_group = if hsm_group_param_opt.is_some()
            && !bos_session_template_hsm_groups
                .iter()
                .any(|h_g| h_g.eq(hsm_group_param_opt.unwrap()))
        {
            eprintln!("HSM group in param does not matches with any HSM groups in SAT file under session_templates.bos_parameters.boot_sets.compute.node_groups section. Using HSM group in param as the default");
            hsm_group_param_opt.unwrap()
        } else {
            bos_session_template_hsm_groups.first().unwrap()
        };

        let create_bos_session_template_payload =
            mesa::bos::template::mesa::r#struct::request_payload::BosSessionTemplate::new_for_hsm_group(
                bos_session_template_configuration_name,
                bos_session_template_name,
                ims_image_name,
                ims_image_path.to_string(),
                ims_image_type.to_string(),
                ims_image_etag.to_string(),
                hsm_group,
            );

        let create_bos_session_template_resp = mesa::bos::template::shasta::http_client::post(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            &create_bos_session_template_payload,
        )
        .await;

        if create_bos_session_template_resp.is_err() {
            eprintln!("BOS session template creation failed");
            std::process::exit(1);
        }

        // Create BOS session. Note: reboot operation shuts down the nodes and they may not start
        // up... hence we will split the reboot into 2 operations shutdown and start

        if do_not_reboot {
            log::info!("Reboot canceled by user");
        } else {
            log::info!("Rebooting nodes");
            // Get nodes members of HSM group
            // Get HSM group details
            let hsm_group_details = hsm::group::shasta::http_client::get(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                Some(hsm_group),
            )
            .await;

            // Get list of xnames in HSM group
            let nodes: Vec<String> = hsm_group_details.unwrap().first().unwrap()["members"]["ids"]
                .as_array()
                .unwrap()
                .iter()
                .map(|node| node.as_str().unwrap().to_string())
                .collect();

            // Create CAPMC operation shutdown
            let capmc_shutdown_nodes_resp = capmc::http_client::node_power_off::post_sync(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                nodes.clone(),
                Some("Shut down cluster to apply changes".to_string()),
                true,
            )
            .await;

            log::debug!(
                "CAPMC shutdown nodes response:\n{:#?}",
                capmc_shutdown_nodes_resp
            );

            // Create BOS session operation start
            let create_bos_boot_session_resp = mesa::bos::session::shasta::http_client::post(
                shasta_token,
                shasta_base_url,
                shasta_root_cert,
                &create_bos_session_template_payload.name,
                "boot",
                Some(&nodes.join(",")),
            )
            .await;

            log::debug!(
                "Create BOS boot session response:\n{:#?}",
                create_bos_boot_session_resp
            );

            if create_bos_boot_session_resp.is_err() {
                eprintln!("Error creating BOS boot session. Exit");
                std::process::exit(1);
            }
        }

        // Audit
        let jwt_claims = get_claims_from_jwt_token(shasta_token).unwrap();

        log::info!(target: "app::audit", "User: {} ({}) ; Operation: Apply cluster", jwt_claims["name"].as_str().unwrap(), jwt_claims["preferred_username"].as_str().unwrap());
    }
}

pub async fn wait_cfs_session_to_complete(
    shasta_token: &str,
    shasta_base_url: &str,
    shasta_root_cert: &[u8],
    cfs_session: &CfsSessionGetResponse,
) {
    // Monitor CFS image creation process ends
    let mut i = 0;
    let max = 1800; // Max ammount of attempts to check if CFS session has ended
    loop {
        let cfs_session_vec = mesa::cfs::session::mesa::http_client::get(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            Some(&cfs_session.name.as_ref().unwrap().to_string()),
            Some(true),
        )
        .await
        .unwrap();
        /*
        mesa::cfs::session::mesa::utils::filter_by_hsm(
            shasta_token,
            shasta_base_url,
            shasta_root_cert,
            &mut cfs_session_vec,
            hsm_group_available_vec,
            Some(&1),
        )
        .await; */

        if !cfs_session_vec.is_empty()
            && cfs_session_vec
                .first()
                .unwrap()
                .status
                .as_ref()
                .unwrap()
                .session
                .as_ref()
                .unwrap()
                .status
                .as_ref()
                .unwrap()
                .eq("complete")
            && i <= max
        {
            /* let cfs_session_aux: CfsSessionGetResponse =
            CfsSessionGetResponse::from_csm_api_json(
                cfs_session_value_vec.first().unwrap().clone(),
            ); */

            let cfs_session = cfs_session_vec.first().unwrap();

            if !cfs_session.is_success() {
                // CFS session failed
                eprintln!(
                    "ERROR creating CFS session '{}'",
                    cfs_session.name.as_ref().unwrap()
                );
                std::process::exit(1);
            } else {
                // CFS session succeeded
                // cfs_session_complete_vec.push(cfs_session.clone());

                log::info!(
                    "CFS session created: {}",
                    cfs_session.name.as_ref().unwrap()
                );

                break;
            }
        } else {
            print!("\x1B[2K");
            io::stdout().flush().unwrap();
            print!(
                "\rCFS session '{}' running. Checking again in 2 secs. Attempt {} of {}",
                cfs_session.name.as_ref().unwrap(),
                i + 1,
                max
            );
            io::stdout().flush().unwrap();

            tokio::time::sleep(time::Duration::from_secs(2)).await;
            std::io::Write::flush(&mut std::io::stdout()).unwrap();

            i += 1;
        }
    }
}
