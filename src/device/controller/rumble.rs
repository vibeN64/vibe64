use crate::device;

pub fn read(device: &mut device::Device, _channel: usize, address: u16, data: usize, size: usize) {
    let value: u8 = if (0x8000..0x9000).contains(&address) {
        0x80
    } else {
        0x00
    };

    for i in 0..size {
        device.pif.ram[data + i] = value;
    }
}

pub fn write(device: &mut device::Device, channel: usize, address: u16, data: usize, size: usize) {
    if address == 0xc000 {
        let rumble = device.pif.ram[data + size - 1];

        if let Some(netplay) = &device.netplay {
            if netplay.player_number == channel {
                device::ui::input::set_rumble(&device.ui, 0, rumble);
            }
        } else {
            device::ui::input::set_rumble(&device.ui, channel, rumble);
        }
    }
}

/// Whether a game should start with a Rumble Pak in the controller instead of a Memory Pak.
///
/// A controller holds one pak at a time. The Rumble Pak goes to games that support it and
/// do not need a Memory Pak to save: they save to the cartridge, or do not save at all.
/// Games that support rumble but can only save to a Memory Pak keep the Memory Pak; the
/// pak can still be changed while playing (hotkey + B).
///
/// The list is taken from the game table of the ares emulator (mia/medium/nintendo-64.cpp),
/// Copyright (c) 2004-2025 ares team, Near et al, used under the ISC license.
pub fn prefers_rumble_pak(rom: &[u8], game_id: &str, port: usize) -> bool {
    // Homebrew says what it wants in each port in its header (0x01 is a Rumble Pak)
    if rom[0x3C..0x3E] == *b"ED" {
        return rom[0x34 + port] == 0x01;
    }

    let japanese = rom[0x3E] == b'J';
    let revision = rom[0x3F];
    match game_id {
        "N3H" | // Ganbare! Nippon! Olympics 2000; elsewhere it saves to the Memory Pak
        "ND3" | // Akumajou Dracula Mokushiroku; Castlevania has no rumble elsewhere
        "ND4"   // Akumajou Dracula Mokushiroku Gaiden: Legend of Cornell
        => japanese,
        "NSM" => japanese && revision == 3, // Super Mario 64 Shindou Edition
        "NWR" => japanese && revision == 2, // Wave Race 64 Shindou Edition
        "NK4" | // Kirby 64: The Crystal Shards [Hoshi no Kirby 64 (J)]
        "NOS" | // 64 Oozumou
        "NTC" | // 64 Trump Collection
        "NER" | // Aero Fighters Assault [Sonic Wings Assault (J)]
        "NAB" | // Air Boarder 64
        "NBK" | // Banjo-Kazooie [Banjo to Kazooie no Daiboken (J)]
        "NFH" | // In-Fisherman Bass Hunter 64
        "NMU" | // Big Mountain 2000
        "NBH" | // Body Harvest
        "NBV" | // Bomberman 64: The Second Attack! [Baku Bomberman 2 (J)]
        "NBD" | // Bomberman Hero [Mirian Ojo o Sukue! (J)]
        "NCT" | // Chameleon Twist
        "NCH" | // Chopper Attack
        "NCG" | // Choro Q 64 II - Hacha Mecha Grand Prix Race (J)
        "NP2" | // Chou Kuukan Night Pro Yakyuu King 2 (J)
        "NXO" | // Cruis'n Exotica
        "NDY" | // Diddy Kong Racing
        "NDU" | // Duck Dodgers starring Daffy Duck
        "NFW" | // F-1 World Grand Prix
        "NF2" | // F-1 World Grand Prix II
        "NKA" | // Fighters Destiny [Fighting Cup (J)]
        "NFG" | // Fighter Destiny 2
        "NGL" | // Getter Love!!
        "NGE" | // GoldenEye 007
        "NPG" | // Hey You, Pikachu! [Pikachu Genki Dechu (J)]
        "NIJ" | // Indiana Jones and the Infernal Machine
        "NIC" | // Indy Racing 2000
        "NFY" | // Kakutou Denshou: F-Cup Maniax
        "NLL" | // Last Legion UX
        "NLR" | // Lode Runner 3-D
        "CLB" | // Mario Party (NTSC)
        "NLB" | // Mario Party (PAL)
        "NMW" | // Mario Party 2
        "NML" | // Mickey's Speedway USA [Mickey no Racing Challenge USA (J)]
        "NMI" | // Mission: Impossible
        "NMG" | // Monaco Grand Prix [Racing Simulation 2 (G)]
        "NMR" | // Multi-Racing Championship
        "NPY" | // Puyo Puyo Sun 64
        "NPT" | // Puyo Puyon Party
        "NRA" | // Rally '99 (J)
        "NWQ" | // Rally Challenge 2000
        "NSU" | // Rocket: Robot on Wheels
        "NSN" | // Snow Speeder (J)
        "NK2" | // Snowboard Kids 2 [Chou Snobow Kids (J)]
        "NSV" | // Space Station Silicon Valley
        "NFX" | // Star Fox 64 [Lylat Wars (E)]
        "NS6" | // Star Soldier: Vanishing Earth
        "NNA" | // Star Wars Episode I: Battle for Naboo
        "NRS" | // Star Wars: Rogue Squadron [Shutsugeki! Rogue Chuutai (J)]
        "NSA" | // Sonic Wings Assault (J)
        "NSS" | // Super Robot Spirits
        "NTX" | // Taz Express
        "NTJ" | // Tom & Jerry in Fists of Fury
        "NRC" | // Top Gear Overdrive
        "NTR" | // Top Gear Rally (J + E)
        "NTB" | // Transformers: Beast Wars Metals 64 (J)
        "NGU" | // Tsumi to Batsu: Hoshi no Keishousha (Sin and Punishment)
        "NIR" | // Utchan Nanchan no Hono no Challenger: Denryuu Ira Ira Bou
        "NVL" | // V-Rally Edition '99
        "NVY" | // V-Rally Edition '99 (J)
        "NJK" | // Viewpoint 2064 (Master)
        "NWC" | // Wild Choppers
        "NYK" | // Yakouchuu II: Satsujin Kouro
        "NB7" | // Banjo-Tooie [Banjo to Kazooie no Daiboken 2 (J)]
        "NGT" | // City Tour GrandPrix - Zen Nihon GT Senshuken
        "NFU" | // Conker's Bad Fur Day
        "NCW" | // Cruis'n World
        "NCZ" | // Custom Robo V2
        "ND6" | // Densha de Go! 64
        "NDO" | // Donkey Kong 64
        "ND2" | // Doraemon 2: Nobita to Hikari no Shinden
        "N3D" | // Doraemon 3: Nobita no Machi SOS!
        "NMX" | // Excitebike 64
        "NGC" | // GT 64: Championship Edition
        "NNB" | // Kobe Bryant in NBA Courtside
        "NMV" | // Mario Party 3
        "NM8" | // Mario Tennis
        "NEV" | // Neon Genesis Evangelion
        "NPD" | // Perfect Dark
        "NRZ" | // Ridge Racer 64
        "NEP" | // Star Wars Episode I: Racer
        "NYS" | // Yoshi's Story
        "NTE" | // 1080 Snowboarding
        "NVB" | // Bass Rush - ECOGEAR PowerWorm Championship (J)
        "NB5" | // Biohazard 2 (J)
        "CFZ" | // F-Zero X (J)
        "NFZ" | // F-Zero X (U + E)
        "NG6" | // Ganmare Goemon: Dero Dero Douchuu Obake Tenkomori
        "NHY" | // Hybrid Heaven (J)
        "NIB" | // Itoi Shigesato no Bass Tsuri No. 1 Kettei Ban!
        "NPS" | // Jikkyou J.League 1999: Perfect Striker 2
        "NJG" | // Jinsei Game 64
        "CZL" | // Legend of Zelda: Ocarina of Time [Zelda no Densetsu - Toki no Ocarina (J)]
        "NZL" | // Legend of Zelda: Ocarina of Time (E)
        "NKG" | // Major League Baseball featuring Ken Griffey Jr.
        "NMF" | // Mario Golf 64
        "NUT" | // Nushi Zuri 64
        "NUM" | // Nushi Zuri 64: Shiokaze ni Notte
        "NRE" | // Resident Evil 2
        "NAL" | // Super Smash Bros. [Nintendo All-Star! Dairantou Smash Brothers (J)]
        "NA2" | // Virtual Pro Wrestling 2
        "NVP" | // Virtual Pro Wrestling 64
        "NWL" | // Waialae Country Club: True Golf Classics
        "NW2" | // WCW-nWo Revenge
        "NWX" | // WWF WrestleMania 2000
        "CDZ" | // Dezaemon 3D
        "NCC" | // Command & Conquer
        "NJF" | // Jet Force Gemini [Star Twins (J)]
        "NKJ" | // Ken Griffey Jr.'s Slugfest
        "NZS" | // Legend of Zelda: Majora's Mask [Zelda no Densetsu - Mujura no Kamen (J)]
        "NM6" | // Mega Man 64
        "NCK" | // NBA Courtside 2 featuring Kobe Bryant
        "NMQ" | // Paper Mario
        "NRH" | // Rockman Dash - Hagane no Boukenshin (J)
        "NSQ" | // StarCraft 64
        "NW4" | // WWF No Mercy
        "NJQ" | // Batman Beyond - Return of the Joker [Batman of the Future - Return of the Joker (E)]
        "NCB" | // Charlie Blast's Territory
        "NDF" | // Dance Dance Revolution - Disney Dancing Museum
        "NKE" | // Knife Edge - Nose Gunner
        "NMT" | // Magical Tetris Challenge
        "NM3" | // Monster Truck Madness 64
        "NRG" | // Rugrats - Scavenger Hunt [Treasure Hunt (E)]
        "NOH" | // Transformers Beast Wars - Transmetals
        "NWF"   // Wheel of Fortune
        => true,
        _ => false,
    }
}
