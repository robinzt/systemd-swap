use std::convert::TryInto;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::Command;
use std::{fs, io, thread, time};
use sysinfo::{RefreshKind, System, SystemExt};

// config
const SWAPFC_BUF_SIZE: usize = 4194304;
const SWAPFC_FREE_PERC: u8 = 15;
const SWAPFC_REMOVE_FREE_PERC: u8 = 55;
const SWAPFC_CHUNK_SIZE: usize = 268435456;
const SWAPFC_MAX_COUNT: u8 = 32;
const SWAPFC_MIN_COUNT: u8 = 0;
//const SWAPFC_PATH: &str = "/var/local/systemd-swap/swapfc";
const SWAPFC_PATH: &str = "/var/lib/systemd-swap/swapfc";
//const RUN_PATH: &str = "/var/local/systemd-swap";
const RUN_PATH: &str = "/run/systemd/swap";

fn main() {
    let lock = RUN_PATH.to_owned() + "/swapfc/.lock";
    let mut allocated: u8 = 0;
    for _ in 0..SWAPFC_MIN_COUNT {
        create_swapfile(&mut allocated);
    }
    swapfc_init(&lock).expect("Unable to initialize program");
    while Path::new(&lock).exists() {
        thread::sleep(time::Duration::from_secs(1));
        if allocated == 0 {
            let curr_free_ram_perc = get_free_ram_perc();
            if curr_free_ram_perc < SWAPFC_FREE_PERC {
                create_swapfile(&mut allocated);
            }
            continue;
        }
        let curr_free_swap_perc = get_free_swap_perc();
        if curr_free_swap_perc < SWAPFC_FREE_PERC && allocated < SWAPFC_MAX_COUNT {
            create_swapfile(&mut allocated);
        }
        if allocated <= 2 || allocated <= SWAPFC_MIN_COUNT {
            continue;
        }
        if curr_free_swap_perc < SWAPFC_REMOVE_FREE_PERC {
            destroy_swapfile(&mut allocated).expect("Unable to remove swap file");
        }
    }
}

fn swapfc_init(lock: &str) -> io::Result<()> {
    fs::create_dir_all(SWAPFC_PATH).expect("Unable to create swapfc_path");
    //fs::create_dir("/run/systemd/swap/swapfc").expect("Unable to create swapfc_run dir");
    fs::File::create(lock).expect("Unable to create swapfc lock");
    Ok(())
}

fn get_free_ram_perc() -> u8 {
    let s = System::new_with_specifics(RefreshKind::new().with_memory());
    let total = s.get_total_memory();
    let free = s.get_free_memory();
    ((free * 100) / total).try_into().unwrap()
}

fn get_free_swap_perc() -> u8 {
    let s = System::new_with_specifics(RefreshKind::new().with_memory());
    let total = s.get_total_swap();
    let free = s.get_free_swap();
    ((free * 100) / total).try_into().unwrap()
}

fn create_swapfile(allocated: &mut u8) {
    //if check_ENOSPC(swapfc_path)
    sd_notify::notify(
        true,
        &[sd_notify::NotifyState::Status(String::from(
            "Allocating swap file...",
        ))],
    )
    .expect("Unable to notify systemd");
    *allocated += 1;
    prepare_swapfile(*allocated).expect("Unable to prepare swap file");
    Command::new("/usr/bin/mkswap")
        .arg(Path::new(SWAPFC_PATH).join(allocated.to_string()))
        .output()
        .expect("Unable to mkswap");
    Command::new("/usr/bin/swapon")
        .arg(Path::new(SWAPFC_PATH).join(allocated.to_string()))
        .output()
        .expect("Unable to swapon");
    sd_notify::notify(
        true,
        &[sd_notify::NotifyState::Status(String::from(
            "Monitoring memory status...",
        ))],
    )
    .expect("Unable to notify systemd");
}

fn prepare_swapfile(file: u8) -> io::Result<()> {
    let file = Path::new(SWAPFC_PATH).join(file.to_string());
    // create swap file
    let mut dst = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(&file)
        .unwrap();
    // create a 4MiB buffer of zeroes
    let buffer = vec![0; SWAPFC_BUF_SIZE];
    // write <SWAPFC_CHUNK_SIZE> to swap file
    for _ in (0..SWAPFC_CHUNK_SIZE).step_by(SWAPFC_BUF_SIZE) {
        // write 4MiB at a time
        dst.write_all(&buffer).expect("Unable to write to file");
    }
    Ok(())
}

fn destroy_swapfile(allocated: &mut u8) -> io::Result<()> {
    sd_notify::notify(
        true,
        &[sd_notify::NotifyState::Status(String::from(
            "Deallocating swap file...",
        ))],
    )
    .expect("Unable to notify systemd");
    Command::new("/usr/bin/swapoff")
        .arg(Path::new(SWAPFC_PATH).join(allocated.to_string()))
        .output()
        .expect("Unable to swapoff");
    fs::remove_file(allocated.to_string()).expect("Unable to remove file");
    *allocated -= 1;
    sd_notify::notify(
        true,
        &[sd_notify::NotifyState::Status(String::from(
            "Monitoring memory status...",
        ))],
    )
    .expect("Unable to notify systemd");
    Ok(())
}

/*
fn check_ENOSPC(x) {
}
*/