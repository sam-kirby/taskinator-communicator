use std::{
    convert::TryInto,
    ffi::c_void,
    mem::{size_of, MaybeUninit},
};

use winapi::{
    shared::minwindef::HMODULE,
    um::{
        errhandlingapi::GetLastError,
        memoryapi::ReadProcessMemory,
        processthreadsapi::OpenProcess,
        psapi::{EnumProcessModulesEx, GetModuleBaseNameW},
        winnt::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    },
};

use crate::{error::Error, Result};

type GameUSize = u32;

pub struct Game {
    handle: usize,
    ga_addr: GameUSize,
}

#[derive(Debug, Clone)]
pub enum State {
    Menu,
    Lobby {
        // code: String,
        players: Vec<Player>,
    },
    InGame {
        // code: String,
        meeting: MeetingState,
        players: Vec<Player>,
        tasks_completed: GameUSize,
        tasks_total: GameUSize,
    },
}

#[repr(u32)]
#[derive(Debug, Clone)]
pub enum MeetingState {
    Discussion,
    NotVoted,
    Voted,
    Results,
    Proceeding,
}

#[derive(Debug, Clone)]
pub struct Player {
    id: u8,
    pub name: String,
    pub colour: i32,
    hat: u32,
    pet: u32,
    skin: u32,
    pub disconnected: bool,
    tasks_addr: GameUSize,
    pub impostor: bool,
    pub dead: bool,
    game_object_addr: GameUSize,
}

#[repr(u32)]
#[allow(dead_code)]
enum InternalState {
    NotJoined,
    Joined,
    Started,
    Ended,
}

impl Game {
    pub fn from_pid(pid: usize) -> Result<Self> {
        const MAX_MODULE_COUNT: usize = 128;
        const MAX_MODULE_NAME_LEN: usize = 64;

        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false.into(),
                pid as u32,
            )
        };

        let mut modules: Vec<HMODULE> = Vec::with_capacity(MAX_MODULE_COUNT);
        let mut count_bytes = 0;

        let enum_modules_result = unsafe {
            EnumProcessModulesEx(
                handle,
                modules.as_mut_ptr(),
                size_of::<isize>() as u32 * MAX_MODULE_COUNT as u32,
                &mut count_bytes,
                0x03, // Get both 32- and 64-bit modules
            )
        };

        unsafe { modules.set_len(count_bytes as usize / size_of::<isize>()) };

        if enum_modules_result == 0 {
            return Err(Error::EnumModuleError(unsafe { GetLastError() }).into());
        }

        let ga_addr = modules.into_iter().find_map(|hm| {
            let mut mod_name: Vec<u16> = Vec::with_capacity(MAX_MODULE_NAME_LEN);

            let len = unsafe {
                GetModuleBaseNameW(
                    handle,
                    hm,
                    mod_name.as_mut_ptr(),
                    MAX_MODULE_NAME_LEN as u32,
                )
            } as usize;

            unsafe { mod_name.set_len(len) };

            if String::from_utf16(&mod_name).ok()? != "GameAssembly.dll" {
                return None;
            }

            Some(hm)
        });

        if let Some(ga_addr) = ga_addr.map(|addr| addr as u32) {
            Ok(Game {
                handle: handle as usize,
                ga_addr,
            })
        } else {
            Err(Error::MissingGaError.into())
        }
    }

    pub fn state(&self) -> Result<State> {
        let client_state_addr = self.get_instance_addr::<ClientState>()?;

        let internal_state = unsafe { self.read_internal_state(client_state_addr) }?;

        match internal_state {
            InternalState::NotJoined => Ok(State::Menu),
            InternalState::Joined | InternalState::Ended => {
                let player_manager_addr = self.get_instance_addr::<PlayerManager>()?;
                let players = unsafe { self.read_players(player_manager_addr) }?;
                Ok(State::Lobby { players })
            }
            InternalState::Started => {
                let player_manager_addr = self.get_instance_addr::<PlayerManager>()?;

                let (tasks_total, tasks_completed) =
                    unsafe { self.read_task_overview(player_manager_addr) }?;
                let players = unsafe { self.read_players(player_manager_addr) }?;

                let meeting_screen_addr = self.get_instance_addr::<MeetingScreen>()?;

                let meeting = if meeting_screen_addr != 0 {
                    unsafe { self.read_meeting_progress(meeting_screen_addr) }?
                } else {
                    tracing::trace!("meeting_screen nullptr -> assuming proceeding");
                    MeetingState::Proceeding
                };

                Ok(State::InGame {
                    meeting,
                    players,
                    tasks_completed,
                    tasks_total,
                })
            }
        }
    }

    unsafe fn read_internal_state(&self, client_state_addr: GameUSize) -> Result<InternalState> {
        const INTERNAL_STATE_OFFSET: GameUSize = 0x70;

        let mut internal_state = MaybeUninit::<InternalState>::uninit();
        let mut count = 0;

        let read_result = ReadProcessMemory(
            self.handle as *mut c_void,
            (client_state_addr + INTERNAL_STATE_OFFSET) as *mut c_void,
            internal_state.as_mut_ptr() as *mut c_void,
            size_of::<InternalState>(),
            &mut count,
        );

        if read_result == 0 {
            return Err(Error::ReadError(GetLastError(), count, "internal state").into());
        }

        Ok(internal_state.assume_init())
    }

    unsafe fn read_players(&self, player_manager_addr: GameUSize) -> Result<Vec<Player>> {
        const PLAYER_LIST_PTR_OFFSET: GameUSize = 0x24;
        const PLAYER_LIST_SIZE_OFFSET: GameUSize = 0xC;
        const PLAYER_LIST_FIRST_OBJECT: GameUSize = 0x8;
        const PLAYER_ARRAY_OFFSET: GameUSize = 0x10;

        let player_list_addr =
            self.read_game_usize(player_manager_addr + PLAYER_LIST_PTR_OFFSET)?;

        let mut player_count = MaybeUninit::<GameUSize>::uninit();
        let mut count = 0;

        let read_result = ReadProcessMemory(
            self.handle as *mut c_void,
            (player_list_addr + PLAYER_LIST_SIZE_OFFSET) as *mut c_void,
            player_count.as_mut_ptr() as *mut c_void,
            size_of::<GameUSize>(),
            &mut count,
        );

        if read_result == 0 {
            return Err(Error::ReadError(GetLastError(), count, "player list size").into());
        }

        let player_count = player_count.assume_init();

        let first_player_addr = self
            .read_game_usize(player_list_addr + PLAYER_LIST_FIRST_OBJECT)?
            + PLAYER_ARRAY_OFFSET;

        let mut players = Vec::with_capacity(player_count as usize);

        for idx in 0..player_count {
            let player_addr = self
                .read_game_usize(first_player_addr + idx * size_of::<GameUSize>() as GameUSize)?;

            players.push(self.read_player(player_addr)?);
        }

        Ok(players)
    }

    unsafe fn read_player(&self, player_addr: GameUSize) -> Result<Player> {
        const PLAYER_STRUCT_SIZE: usize = 0x2C;
        let mut raw_bytes: Vec<u8> = Vec::with_capacity(PLAYER_STRUCT_SIZE);
        let mut count = 0;

        let read_result = ReadProcessMemory(
            self.handle as *mut c_void,
            (player_addr + 8) as *mut c_void, // + 8 to skip klass/monitor fields
            raw_bytes.as_mut_ptr() as *mut c_void,
            PLAYER_STRUCT_SIZE,
            &mut count,
        );

        if read_result == 0 || count != PLAYER_STRUCT_SIZE {
            return Err(Error::ReadError(GetLastError(), count, "raw player").into());
        }

        raw_bytes.set_len(count);

        let id = raw_bytes[0];
        let name_addr = u32::from_ne_bytes(raw_bytes[4..8].try_into()?);
        let _unknown_bool = raw_bytes[8] != 0;
        let colour = i32::from_ne_bytes(raw_bytes[12..16].try_into()?);
        let hat = u32::from_ne_bytes(raw_bytes[16..20].try_into()?);
        let pet = u32::from_ne_bytes(raw_bytes[20..24].try_into()?);
        let skin = u32::from_ne_bytes(raw_bytes[24..28].try_into()?);
        let disconnected = raw_bytes[28] != 0;
        let tasks_addr = u32::from_ne_bytes(raw_bytes[32..36].try_into()?);
        let impostor = raw_bytes[36] != 0;
        let dead = raw_bytes[37] != 0;
        let game_object_addr = u32::from_ne_bytes(raw_bytes[40..44].try_into()?);

        let name = self.read_string(name_addr)?;

        Ok(Player {
            id,
            name,
            colour,
            hat,
            pet,
            skin,
            disconnected,
            tasks_addr,
            impostor,
            dead,
            game_object_addr,
        })
    }

    unsafe fn read_task_overview(
        &self,
        player_manager_addr: GameUSize,
    ) -> Result<(GameUSize, GameUSize)> {
        const TASKS_OFFSET: GameUSize = 0x28;

        let mut tasks_tuple = MaybeUninit::<(GameUSize, GameUSize)>::uninit();
        let mut count = 0;

        let read_result = ReadProcessMemory(
            self.handle as *mut c_void,
            (player_manager_addr + TASKS_OFFSET) as *mut c_void,
            tasks_tuple.as_mut_ptr() as *mut c_void,
            size_of::<(GameUSize, GameUSize)>(),
            &mut count,
        );

        if read_result == 0 {
            return Err(Error::ReadError(GetLastError(), count, "task overview").into());
        }

        Ok(tasks_tuple.assume_init())
    }

    unsafe fn read_meeting_progress(&self, meeting_screen_addr: GameUSize) -> Result<MeetingState> {
        const MEETING_STATE_OFFSET: GameUSize = 0x84;

        let mut meeting_state = MaybeUninit::<MeetingState>::uninit();
        let mut count = 0;

        let read_result = ReadProcessMemory(
            self.handle as *mut c_void,
            (meeting_screen_addr + MEETING_STATE_OFFSET) as *mut c_void,
            meeting_state.as_mut_ptr() as *mut c_void,
            size_of::<MeetingState>(),
            &mut count,
        );

        if read_result == 0 {
            return Err(Error::ReadError(GetLastError(), count, "meeting state").into());
        }

        Ok(meeting_state.assume_init())
    }

    fn get_instance_addr<T: InstancedClass>(&self) -> Result<GameUSize> {
        let class_addr = unsafe { self.read_game_usize(self.ga_addr + T::CLASS_OFFSET) }?;
        let statics_addr = unsafe { self.read_game_usize(class_addr + T::STATICS_OFFSET) }?;
        let instance_addr = unsafe { self.read_game_usize(statics_addr + T::INSTANCE_OFFSET) }?;

        Ok(instance_addr)
    }

    unsafe fn read_game_usize(&self, address: GameUSize) -> Result<GameUSize> {
        let mut ptr = MaybeUninit::<GameUSize>::uninit();
        let mut count = 0;

        let read_result = ReadProcessMemory(
            self.handle as *mut c_void,
            address as *mut c_void,
            ptr.as_mut_ptr() as *mut c_void,
            size_of::<GameUSize>(),
            &mut count,
        );

        if read_result == 0 {
            return Err(Error::ReadError(GetLastError(), count, "pointer").into());
        }

        Ok(ptr.assume_init())
    }

    unsafe fn read_string(&self, address: GameUSize) -> Result<String> {
        let str_len = self.read_game_usize(address + 0x08)?;
        let mut count = 0;

        let mut str_raw: Vec<u16> = Vec::with_capacity(str_len as usize);
        let read_result = ReadProcessMemory(
            self.handle as *mut c_void,
            (address + 12) as *mut c_void,
            str_raw.as_mut_ptr() as *mut c_void,
            str_len as usize * size_of::<u16>(),
            &mut count,
        );

        if read_result == 0 || count / size_of::<u16>() != str_len as usize {
            return Err(Error::ReadError(GetLastError(), count, "string").into());
        }

        str_raw.set_len(str_len as usize);

        Ok(String::from_utf16(&str_raw)?)
    }
}

trait InstancedClass {
    const CLASS_OFFSET: GameUSize;
    const STATICS_OFFSET: GameUSize = 0x5C;
    const INSTANCE_OFFSET: GameUSize = 0x00;
}

struct ClientState {}

impl InstancedClass for ClientState {
    const CLASS_OFFSET: GameUSize = 0x028E98F4; // AmongUsClient
}

struct PlayerManager {}

impl InstancedClass for PlayerManager {
    const CLASS_OFFSET: GameUSize = 0x0290551C; // GameData
}

struct MeetingScreen {}

impl InstancedClass for MeetingScreen {
    const CLASS_OFFSET: GameUSize = 0x028E25A8; // MeetingHud
}
