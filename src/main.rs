mod cli;
mod common;
use mesa::{common::jwt_ops, shasta};

use shasta::authentication;

use crate::common::log_ops;

// DHAT (profiling)
// #[cfg(feature = "dhat-heap")]
// #[global_allocator]
// static ALOC: dhat::Alloc = dhat::Alloc;

// fn main() {
//     println!("crap");
// }

#[tokio::main]
async fn main() -> core::result::Result<(), Box<dyn std::error::Error>> {
    //println!("async main");
    // DHAT (profiling)
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    /* // XDG Base Directory Specification
    let project_dirs = ProjectDirs::from(
        "local", /*qualifier*/
        "cscs",  /*organization*/
        "manta", /*application*/
    );

    let mut path_to_manta_configuration_file = PathBuf::from(project_dirs.unwrap().config_dir());

    // ~/.config/manta/config.toml on Linux
    // ~/Library/Application Support/local.cscs.manta/config.toml on MacOS
    path_to_manta_configuration_file.push("config.toml");

    if path_to_manta_configuration_file.exists() {
        let k = match File::open(&path_to_manta_configuration_file) {
            Err(_why) => {
                println!(
                    "The configuration file at {} is not readable. Cannot continue.",
                    &path_to_manta_configuration_file.to_string_lossy()
                );
                std::process::exit(exitcode::CONFIG)
            }
            Ok(_file) => log::info!(
                "Reading manta configuration from {}",
                &path_to_manta_configuration_file.to_string_lossy()
            ),
        };
    } else {
        println!(
            "The configuration file at {} does not exist or is not readable. Cannot continue.",
            &path_to_manta_configuration_file.to_string_lossy()
        );
        std::process::exit(exitcode::CONFIG);
    }

    // let settings = config::get_configuration(&path_to_manta_configuration_file.to_string_lossy());
    let settings = ::config::Config::builder()
        .add_source(::config::File::from(path_to_manta_configuration_file))
        .add_source(
            ::config::Environment::with_prefix("MANTA")
                .try_parsing(true)
                .prefix_separator("_"),
        )
        .build()
        .unwrap(); */

    let settings = common::config_ops::get_configuration();

    let shasta_base_url = settings.get_string("shasta_base_url").unwrap();
    let vault_base_url = settings.get_string("vault_base_url").unwrap();
    let vault_role_id = settings.get_string("vault_role_id").unwrap();
    let vault_secret_path = settings.get_string("vault_secret_path").unwrap();
    let gitea_base_url = settings.get_string("gitea_base_url").unwrap();
    let keycloak_base_url = settings.get_string("keycloak_base_url").unwrap();
    let k8s_api_url = settings.get_string("k8s_api_url").unwrap();
    let log_level = settings.get_string("log").unwrap_or("error".to_string());

    // Init logger
    // env_logger::init();
    // log4rs::init_file("log4rs.yml", Default::default()).unwrap(); // log4rs file configuration
    log_ops::configure(log_level); // log4rs programatically configuration

    if let Ok(socks_proxy) = settings.get_string("socks5_proxy") {
        std::env::set_var("SOCKS5", socks_proxy);
    }

    let settings_hsm_group_opt = settings.get_string("hsm_group").ok();
    /* let settings_hsm_available_vec = settings
    .get_array("hsm_available")
    .unwrap_or(Vec::new())
    .into_iter()
    .map(|hsm_group| hsm_group.into_string().unwrap())
    .collect::<Vec<String>>(); */

    let shasta_root_cert = common::config_ops::get_csm_root_cert_content();

    /* let hsm_group = match &settings_hsm_group {
        Ok(hsm_group_val) => {
            /* println!(
                "\nWorking on nodes related to *{}{}{}* hsm groups\n",
                color::Fg(color::Green),
                hsm_group_val,
                color::Fg(color::Reset)
            ); */
            Some(hsm_group_val)
        }
        Err(_) => None,
    }; */

    let shasta_token =
        authentication::get_api_token(&shasta_base_url, &shasta_root_cert, &keycloak_base_url)
            .await?;

    let mut settings_hsm_available_vec = jwt_ops::get_claims_from_jwt_token(&shasta_token)
        .unwrap()
        .pointer("/realm_access/roles")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|role_value| role_value.as_str().unwrap().to_string())
        .collect::<Vec<String>>();

    settings_hsm_available_vec
        .retain(|role| !role.eq("offline_access") && !role.eq("uma_authorization"));

    // println!("JWT token resour_access:\n{:?}", realm_access_role_vec);

    // let settings_hsm_available_vec = realm_access_role_vec;

    let gitea_token = crate::common::vault::http_client::fetch_shasta_vcs_token(
        &vault_base_url,
        &vault_secret_path,
        &vault_role_id,
    )
    .await
    .unwrap();

    // Process input params
    let matches =
        crate::cli::build::build_cli(settings_hsm_group_opt.as_ref(), &settings_hsm_available_vec)
            .get_matches();
    let cli_result = crate::cli::process::process_cli(
        matches,
        &shasta_token,
        &shasta_base_url,
        &shasta_root_cert,
        &vault_base_url,
        &vault_secret_path,
        &vault_role_id,
        &gitea_token,
        &gitea_base_url,
        settings_hsm_group_opt.as_ref(),
        &settings_hsm_available_vec.clone(),
        // &base_image_id,
        &k8s_api_url,
    )
    .await;

    match cli_result {
        Ok(_) => Ok(()),
        Err(e) => panic!("{}", e),
    }
}
