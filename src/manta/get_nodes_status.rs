use crate::shasta::{self, hsm};

pub async fn exec(shasta_token: &str, shasta_base_url: &str, hsm_groups_node_list: Vec<String>) -> Vec<Vec<String>> {

    let hsm_group_nodes_string = hsm_groups_node_list.join(",");

    let components_status = shasta::cfs::component::http_client::get_multiple_components(
        shasta_token,
        shasta_base_url,
        Some(&hsm_group_nodes_string),
        None,
    )
    .await
    .unwrap();

    // get boot params
    let nodes_boot_params_list = shasta::bss::http_client::get_boot_params(
        shasta_token,
        shasta_base_url,
        &hsm_groups_node_list,
    )
    .await
    .unwrap();

    // get all cfs configurations so we can link cfs configuration name with its counterpart in the
    // bos sessiontemplate, we are doing this because bos sessiontemplate does not have
    // creation/update time hence i can't sort by date to loop and find out most recent bos
    // sessiontemplate per node. joining cfs configuration and bos sessiontemplate will help to
    // this
    let mut cfs_configuration_list = shasta::cfs::configuration::http_client::get(
        shasta_token,
        shasta_base_url,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // reverse list in order to have most recent cfs configuration lastupdate values at front
    cfs_configuration_list.reverse();

    // println!("bos_sessiontemplate_list:\n{:#?}", bos_sessiontemplate_list);

    // get nodes details (nids) from hsm
    let nodes_hsm_info_resp = hsm::http_client::get_components_status(
        shasta_token,
        shasta_base_url,
        hsm_groups_node_list.clone(),
    )
    .await
    .unwrap();

    // match node with bot_sessiontemplate and put them in a list
    let mut node_details_list = Vec::new();

    for node in &hsm_groups_node_list {
        // let mut kernel_image_path_in_boot_params = none;
        // let mut manifest_image_path_in_bos_sessiontemplate = none;
        // let mut cfs_configuration_name = none;
        let mut node_details = Vec::new();

        // find component details
        let component_details = components_status
            .iter()
            .find(|component_status| component_status["id"].as_str().unwrap().eq(node))
            .unwrap();

        let desired_configuration = component_details["DesiredConfig"]
            .as_str()
            .unwrap_or_default();
        let configuration_status = component_details["ConfigurationStatus"]
            .as_str()
            .unwrap_or_default();
        let enabled = component_details["enabled"].as_bool().unwrap_or_default();
        let error_count = component_details["ErrorCount"].as_i64().unwrap_or_default();
        // let tags = component_details["tags"].to_string();

        // get power status
        // node_power_status = get_node_power_status(node, &nodes_power_status_resp);
        let node_hsm_info = nodes_hsm_info_resp["Components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|&component| component["ID"].as_str().unwrap().eq(node))
            .unwrap();

        let node_power_status = node_hsm_info["State"]
            .as_str()
            .unwrap()
            .to_string()
            .to_uppercase();

        let node_nid = format!(
            "nid{:0>6}",
            node_hsm_info["NID"].as_u64().unwrap().to_string()
        );

        node_details.push(node.to_string());
        node_details.push(node_nid);
        node_details.push(node_power_status);
        node_details.push(desired_configuration.to_string());
        node_details.push(configuration_status.to_string());
        node_details.push(enabled.to_string());
        node_details.push(error_count.to_string());
        // node_details.push(tags);

        // get node boot params (these are the boot params of the nodes with the image the node
        // boot with). the image in the bos sessiontemplate may be different i don't know why. need
        // to investigate
        let node_boot_params = nodes_boot_params_list.iter().find(|&node_boot_param| {
            node_boot_param["hosts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|host_value| host_value.as_str().unwrap())
                .any(|host| host.eq(node))
        });

        // println!("node_boot_params:\n{:#?}", node_boot_params);

        let kernel_image_path_in_boot_params = Some(
            node_boot_params.unwrap()["kernel"]
                .as_str()
                .unwrap()
                .to_string()
                .trim_start_matches("s3://boot-images/")
                .trim_end_matches("/kernel")
                .to_string()
                .to_owned(),
        )
        .unwrap_or_default();

        node_details.push(kernel_image_path_in_boot_params);

        node_details_list.push(node_details.to_owned());
    }
    let components_status = shasta::cfs::component::http_client::get_multiple_components(
        shasta_token,
        shasta_base_url,
        Some(&hsm_group_nodes_string),
        None,
    )
    .await
    .unwrap();

    // get boot params
    let nodes_boot_params_list = shasta::bss::http_client::get_boot_params(
        shasta_token,
        shasta_base_url,
        &hsm_groups_node_list,
    )
    .await
    .unwrap();

    // get all cfs configurations so we can link cfs configuration name with its counterpart in the
    // bos sessiontemplate, we are doing this because bos sessiontemplate does not have
    // creation/update time hence i can't sort by date to loop and find out most recent bos
    // sessiontemplate per node. joining cfs configuration and bos sessiontemplate will help to
    // this
    let mut cfs_configuration_list = shasta::cfs::configuration::http_client::get(
        shasta_token,
        shasta_base_url,
        None,
        None,
        None,
    )
    .await
    .unwrap();

    // reverse list in order to have most recent cfs configuration lastupdate values at front
    cfs_configuration_list.reverse();

    // println!("bos_sessiontemplate_list:\n{:#?}", bos_sessiontemplate_list);

    // get nodes details (nids) from hsm
    let nodes_hsm_info_resp = hsm::http_client::get_components_status(
        shasta_token,
        shasta_base_url,
        hsm_groups_node_list.clone(),
    )
    .await
    .unwrap();

    // match node with bot_sessiontemplate and put them in a list
    let mut node_details_list = Vec::new();

    for node in &hsm_groups_node_list {
        // let mut kernel_image_path_in_boot_params = none;
        // let mut manifest_image_path_in_bos_sessiontemplate = none;
        // let mut cfs_configuration_name = none;
        let mut node_details = Vec::new();

        // find component details
        let component_details = components_status
            .iter()
            .find(|component_status| component_status["id"].as_str().unwrap().eq(node))
            .unwrap();

        let desired_configuration = component_details["desiredconfig"]
            .as_str()
            .unwrap_or_default();
        let configuration_status = component_details["configurationstatus"]
            .as_str()
            .unwrap_or_default();
        let enabled = component_details["enabled"].as_bool().unwrap_or_default();
        let error_count = component_details["errorcount"].as_i64().unwrap_or_default();
        // let tags = component_details["tags"].to_string();

        // get power status
        // node_power_status = get_node_power_status(node, &nodes_power_status_resp);
        let node_hsm_info = nodes_hsm_info_resp["Components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|&component| component["ID"].as_str().unwrap().eq(node))
            .unwrap();

        let node_power_status = node_hsm_info["State"]
            .as_str()
            .unwrap()
            .to_string()
            .to_uppercase();

        let node_nid = format!(
            "nid{:0>6}",
            node_hsm_info["NID"].as_u64().unwrap().to_string()
        );

        node_details.push(node.to_string());
        node_details.push(node_nid);
        node_details.push(node_power_status);
        node_details.push(desired_configuration.to_string());
        node_details.push(configuration_status.to_string());
        node_details.push(enabled.to_string());
        node_details.push(error_count.to_string());
        // node_details.push(tags);

        // get node boot params (these are the boot params of the nodes with the image the node
        // boot with). the image in the bos sessiontemplate may be different i don't know why. need
        // to investigate
        let node_boot_params = nodes_boot_params_list.iter().find(|&node_boot_param| {
            node_boot_param["hosts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|host_value| host_value.as_str().unwrap())
                .any(|host| host.eq(node))
        });

        // println!("node_boot_params:\n{:#?}", node_boot_params);

        let kernel_image_path_in_boot_params = Some(
            node_boot_params.unwrap()["kernel"]
                .as_str()
                .unwrap()
                .to_string()
                .trim_start_matches("s3://boot-images/")
                .trim_end_matches("/kernel")
                .to_string()
                .to_owned(),
        )
        .unwrap_or_default();

        node_details.push(kernel_image_path_in_boot_params);

        node_details_list.push(node_details.to_owned());
    }

    node_details_list
}