// В release на Windows не открывать консольное окно.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    school21_desktop_lib::run()
}
