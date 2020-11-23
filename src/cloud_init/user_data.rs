use crate::config;
use crate::utils;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::fs::File;
use std::io;
use std::io::BufReader;

pub trait GenerateUserData {
    fn generate_user_data<W: io::Write>(
        &self,
        hostname: &str,
        group: &str,
        target_state: &str,
        writer: W,
    ) -> Result<()>;
}

pub struct UserDataGenerator {
    config: config::CloudInit,
}

impl UserDataGenerator {
    pub fn new(config: config::CloudInit) -> Self {
        Self { config }
    }
}

impl GenerateUserData for UserDataGenerator {
    fn generate_user_data<W: io::Write>(
        &self,
        hostname: &str,
        group: &str,
        target_state: &str,
        mut writer: W,
    ) -> Result<()> {
        let extra_vars = generate_extra_vars(
            &hostname,
            group,
            target_state,
            &self.config.extra_vars_base_file_path,
        )?;
        let mut cloud_config = read_cloud_config(&self.config.user_data_base_file_path)?;

        for user_data_file in self.config.user_data_files.iter() {
            cloud_config.write_files.push(CloudConfigWriteFile {
                path: user_data_file.destination.clone(),
                encoding: String::from("gz+b64"),
                content: encode_file(&user_data_file.source)?,
            })
        }

        cloud_config.write_files.push(CloudConfigWriteFile {
            path: self.config.extra_vars_destination_path.clone(),
            encoding: String::from("gz+b64"),
            content: encode(extra_vars.as_slice())?,
        });

        writer.write_all(b"#cloud-config\n")?;
        serde_yaml::to_writer(writer, &cloud_config)?;

        Ok(())
    }
}

fn generate_extra_vars(
    hostname: &str,
    group: &str,
    target_state: &str,
    base_file_path: &str,
) -> Result<Vec<u8>> {
    let mut value: Value = File::open(base_file_path)
        .with_context(|| format!("Failed to open extra vars base file {}", base_file_path))
        .map(BufReader::new)
        .and_then(|r| {
            serde_yaml::from_reader(r).with_context(|| "Failed to parse extra vars base file")
        })?;

    if !value.is_mapping() {
        return Err(anyhow!(
            "Root object in extra vars has to be a mapping but is currently: {}",
            utils::type_name_val(&value)
        ));
    }

    let mapping = value.as_mapping_mut().unwrap();
    mapping.insert(
        Value::String(String::from("hostname")),
        Value::String(String::from(hostname)),
    );
    mapping.insert(
        Value::String(String::from("node_group")),
        Value::String(String::from(group)),
    );
    mapping.insert(
        Value::String(String::from("node_state")),
        Value::String(String::from(target_state)),
    );

    let mut extra_vars = Vec::new();
    serde_yaml::to_writer(&mut extra_vars, &mapping)?;

    Ok(extra_vars)
}

fn read_cloud_config(path: &str) -> Result<CloudConfig> {
    File::open(path)
        .with_context(|| format!("Failed to open cloud config file {}", path))
        .map(BufReader::new)
        .and_then(|r| serde_yaml::from_reader(r).with_context(|| "Failed to parse cloud config"))
}

pub fn encode_file(path: &str) -> Result<String> {
    let file_reader = File::open(path)
        .with_context(|| format!("Failed to open file for encoding {}", path))
        .map(BufReader::new)?;

    encode(file_reader)
}

pub fn encode<R: io::Read>(mut reader: R) -> Result<String> {
    let mut output = vec![];

    {
        let base64 = base64::write::EncoderWriter::new(&mut output, base64::STANDARD);
        let mut gzip = libflate::gzip::Encoder::new(base64)?;

        io::copy(&mut reader, &mut gzip)?;

        let mut base64 = gzip.finish().into_result()?;
        base64.finish()?;
    }

    Ok(String::from_utf8(output)?)
}

#[derive(Deserialize, Serialize)]
struct CloudConfig {
    package_upgrade: bool,
    packages: Vec<String>,
    write_files: Vec<CloudConfigWriteFile>,
    runcmd: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct CloudConfigWriteFile {
    path: String,
    encoding: String,
    content: String,
}
