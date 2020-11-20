use crate::config;
use anyhow::{Context, Result};
use base64_stream::ToBase64Writer;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};

pub trait GenerateUserData {
    fn generate_user_data<W: io::Write>(&self, hostname: &str, writer: W) -> Result<()>;
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
    fn generate_user_data<W: io::Write>(&self, hostname: &str, mut writer: W) -> Result<()> {
        let extra_vars = generate_extra_vars(&hostname, &self.config.extra_vars_base_file_path)?;
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

        writer.write_all("#cloud-config\n".as_bytes())?;
        serde_yaml::to_writer(writer, &cloud_config)?;

        Ok(())
    }
}

fn generate_extra_vars(hostname: &str, base_file_path: &str) -> Result<Vec<u8>> {
    let file = File::open(base_file_path)?;
    let hostname_line = format!("\nhostname: {}\n", hostname);

    let mut extra_vars = Vec::with_capacity(file.metadata()?.len() as usize);
    file.chain(hostname_line.as_bytes())
        .read_to_end(&mut extra_vars)?;

    Ok(extra_vars)
}

fn read_cloud_config(path: &str) -> Result<CloudConfig> {
    File::open(path)
        .with_context(|| format!("Failed to open file {}", path))
        .map(|f| BufReader::new(f))
        .and_then(|r| serde_yaml::from_reader(r).with_context(|| "Failed to parse cloud config"))
}

fn encode_file(path: &str) -> Result<String> {
    let file_reader = File::open(path).map(BufReader::new)?;

    encode(file_reader)
}

fn encode<R: io::Read>(mut reader: R) -> Result<String> {
    let mut base64_content = vec![];
    let base64_writer = ToBase64Writer::new(&mut base64_content);
    let mut gzip_writer = libflate::gzip::Encoder::new(base64_writer)?;

    io::copy(&mut reader, &mut gzip_writer)?;
    gzip_writer.finish().into_result()?;

    Ok(String::from_utf8(base64_content)?)
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
