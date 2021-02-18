fn main() {
    windows::build!(
        windows::win32::debug::GetLastError
        windows::win32::debug::ReadProcessMemory
        windows::win32::process_status::K32EnumProcessModulesEx
        windows::win32::process_status::K32GetModuleBaseNameW
        windows::win32::system_services::HANDLE
        windows::win32::system_services::OpenProcess
        windows::win32::windows_programming::ProcessAccessRights
    );
}
