use comfy_table::Table;
use mesa::manta::cfs::configuration::Configuration;
use serde_json::Value;

pub fn print_table_value(cfs_configuration_value_vec: &Vec<Value>) {
    let mut table = Table::new();

    table.set_header(vec!["Name", "Last Updated", "Layers"]);

    for cfs_configuration_value in cfs_configuration_value_vec {
        let mut layers: Vec<String> = Vec::new();

        if cfs_configuration_value.get("layers").is_some()
            && cfs_configuration_value["layers"].is_array()
        {
            let cfs_configuration_layer_value_vec =
                cfs_configuration_value["layers"].as_array().unwrap();

            let mut i = 0;
            for cfs_configuration_layer_value in cfs_configuration_layer_value_vec {
                println!(
                    "cfs_configuration_layer_value: {}",
                    cfs_configuration_layer_value
                );
                layers.push(format!(
                    "Layer {}:\n - commit id: {}\n - branch: {}n\n - name: {}\n - clone url: {}\n - playbook: {}",
                    i,
                    cfs_configuration_layer_value["commit"].as_str().unwrap(),
                    cfs_configuration_layer_value["branch"].as_str().unwrap(),
                    cfs_configuration_layer_value["name"].as_str().unwrap(),
                    cfs_configuration_layer_value["cloneUrl"].as_str().unwrap(),
                    cfs_configuration_layer_value["playbook"].as_str().unwrap(),
                ));

                i += 1;
            }
        }

        table.add_row(vec![
            cfs_configuration_value["name"]
                .as_str()
                .unwrap()
                .to_string(),
            cfs_configuration_value["lastUpdated"]
                .as_str()
                .unwrap()
                .to_string(),
            layers.join("\n--------------------------\n").to_string(),
        ]);
    }

    println!("{table}");
}

pub fn print_table_struct(cfs_configuration: Configuration) {
    let mut table = Table::new();

    table.set_header(vec!["Name", "Last updated", "Layers"]);

    let mut layers: String = String::new();

    if !cfs_configuration.config_layers.is_empty() {
        layers = format!(
            "commit id: {} commit date: {} name: {} author: {}",
            cfs_configuration.config_layers[0].commit_id,
            cfs_configuration.config_layers[0].commit_date,
            cfs_configuration.config_layers[0].name,
            cfs_configuration.config_layers[0].author
        );

        for i in 1..cfs_configuration.config_layers.len() {
            let layer = &cfs_configuration.config_layers[i];
            layers = format!(
                "{}\ncommit id: {} commit date: {} name: {} author: {}",
                layers, layer.commit_id, layer.commit_date, layer.name, layer.author
            );
        }
    }

    table.add_row(vec![
        cfs_configuration.name,
        cfs_configuration.last_updated,
        layers,
    ]);

    println!("{table}");
}