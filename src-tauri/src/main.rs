#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    smc_downloader_lib::run()
}
