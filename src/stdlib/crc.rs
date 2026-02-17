use crate::compiler::function::EnumVariant;
use crate::compiler::prelude::*;
use crc::Crc as CrcInstance;
use std::sync::LazyLock;

static DEFAULT_ALGORITHM: LazyLock<Value> =
    LazyLock::new(|| Value::Bytes(Bytes::from("CRC_32_ISO_HDLC")));

const VALID_ALGORITHMS: &[&str] = &[
    "CRC_3_GSM",
    "CRC_3_ROHC",
    "CRC_4_G_704",
    "CRC_4_INTERLAKEN",
    "CRC_5_EPC_C1G2",
    "CRC_5_G_704",
    "CRC_5_USB",
    "CRC_6_CDMA2000_A",
    "CRC_6_CDMA2000_B",
    "CRC_6_DARC",
    "CRC_6_GSM",
    "CRC_6_G_704",
    "CRC_7_MMC",
    "CRC_7_ROHC",
    "CRC_7_UMTS",
    "CRC_8_AUTOSAR",
    "CRC_8_BLUETOOTH",
    "CRC_8_CDMA2000",
    "CRC_8_DARC",
    "CRC_8_DVB_S2",
    "CRC_8_GSM_A",
    "CRC_8_GSM_B",
    "CRC_8_HITAG",
    "CRC_8_I_432_1",
    "CRC_8_I_CODE",
    "CRC_8_LTE",
    "CRC_8_MAXIM_DOW",
    "CRC_8_MIFARE_MAD",
    "CRC_8_NRSC_5",
    "CRC_8_OPENSAFETY",
    "CRC_8_ROHC",
    "CRC_8_SAE_J1850",
    "CRC_8_SMBUS",
    "CRC_8_TECH_3250",
    "CRC_8_WCDMA",
    "CRC_10_ATM",
    "CRC_10_CDMA2000",
    "CRC_10_GSM",
    "CRC_11_FLEXRAY",
    "CRC_11_UMTS",
    "CRC_12_CDMA2000",
    "CRC_12_DECT",
    "CRC_12_GSM",
    "CRC_12_UMTS",
    "CRC_13_BBC",
    "CRC_14_DARC",
    "CRC_14_GSM",
    "CRC_15_CAN",
    "CRC_15_MPT1327",
    "CRC_16_ARC",
    "CRC_16_CDMA2000",
    "CRC_16_CMS",
    "CRC_16_DDS_110",
    "CRC_16_DECT_R",
    "CRC_16_DECT_X",
    "CRC_16_DNP",
    "CRC_16_EN_13757",
    "CRC_16_GENIBUS",
    "CRC_16_GSM",
    "CRC_16_IBM_3740",
    "CRC_16_IBM_SDLC",
    "CRC_16_ISO_IEC_14443_3_A",
    "CRC_16_KERMIT",
    "CRC_16_LJ1200",
    "CRC_16_M17",
    "CRC_16_MAXIM_DOW",
    "CRC_16_MCRF4XX",
    "CRC_16_MODBUS",
    "CRC_16_NRSC_5",
    "CRC_16_OPENSAFETY_A",
    "CRC_16_OPENSAFETY_B",
    "CRC_16_PROFIBUS",
    "CRC_16_RIELLO",
    "CRC_16_SPI_FUJITSU",
    "CRC_16_T10_DIF",
    "CRC_16_TELEDISK",
    "CRC_16_TMS37157",
    "CRC_16_UMTS",
    "CRC_16_USB",
    "CRC_16_XMODEM",
    "CRC_17_CAN_FD",
    "CRC_21_CAN_FD",
    "CRC_24_BLE",
    "CRC_24_FLEXRAY_A",
    "CRC_24_FLEXRAY_B",
    "CRC_24_INTERLAKEN",
    "CRC_24_LTE_A",
    "CRC_24_LTE_B",
    "CRC_24_OPENPGP",
    "CRC_24_OS_9",
    "CRC_30_CDMA",
    "CRC_31_PHILIPS",
    "CRC_32_AIXM",
    "CRC_32_AUTOSAR",
    "CRC_32_BASE91_D",
    "CRC_32_BZIP2",
    "CRC_32_CD_ROM_EDC",
    "CRC_32_CKSUM",
    "CRC_32_ISCSI",
    "CRC_32_ISO_HDLC",
    "CRC_32_JAMCRC",
    "CRC_32_MEF",
    "CRC_32_MPEG_2",
    "CRC_32_XFER",
    "CRC_40_GSM",
    "CRC_64_ECMA_182",
    "CRC_64_GO_ISO",
    "CRC_64_MS",
    "CRC_64_REDIS",
    "CRC_64_WE",
    "CRC_64_XZ",
    "CRC_82_DARC",
];

static ALGORITHM_ENUM: &[EnumVariant] = &[
    EnumVariant {
        value: "CRC_3_GSM",
        description: "3-bit CRC used in GSM telecommunications for error detection",
    },
    EnumVariant {
        value: "CRC_3_ROHC",
        description: "3-bit CRC used in Robust Header Compression (ROHC) protocol",
    },
    EnumVariant {
        value: "CRC_4_G_704",
        description: "4-bit CRC specified in ITU-T G.704 for synchronous communication systems",
    },
    EnumVariant {
        value: "CRC_4_INTERLAKEN",
        description: "4-bit CRC used in Interlaken high-speed serial communication protocol",
    },
    EnumVariant {
        value: "CRC_5_EPC_C1G2",
        description: "5-bit CRC used in EPC Gen 2 RFID (Radio-Frequency Identification) standard",
    },
    EnumVariant {
        value: "CRC_5_G_704",
        description: "5-bit CRC variant in ITU-T G.704 telecommunication standard",
    },
    EnumVariant {
        value: "CRC_5_USB",
        description: "5-bit CRC used in USB communication for detecting transmission errors",
    },
    EnumVariant {
        value: "CRC_6_CDMA2000_A",
        description: "6-bit CRC variant used in CDMA2000 network protocols",
    },
    EnumVariant {
        value: "CRC_6_CDMA2000_B",
        description: "Alternative 6-bit CRC variant for CDMA2000 network protocols",
    },
    EnumVariant {
        value: "CRC_6_DARC",
        description: "6-bit CRC used in DARC (Digital Audio Radio Channel) communication",
    },
    EnumVariant {
        value: "CRC_6_GSM",
        description: "6-bit CRC variant used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_6_G_704",
        description: "6-bit CRC specified in ITU-T G.704 for synchronous communication",
    },
    EnumVariant {
        value: "CRC_7_MMC",
        description: "7-bit CRC used in MultiMediaCard (MMC) storage systems for error detection",
    },
    EnumVariant {
        value: "CRC_7_ROHC",
        description: "7-bit CRC used in Robust Header Compression (ROHC) protocol",
    },
    EnumVariant {
        value: "CRC_7_UMTS",
        description: "7-bit CRC used in UMTS (Universal Mobile Telecommunications System)",
    },
    EnumVariant {
        value: "CRC_8_AUTOSAR",
        description: "8-bit CRC used in AUTOSAR (Automotive Open System Architecture) standard",
    },
    EnumVariant {
        value: "CRC_8_BLUETOOTH",
        description: "8-bit CRC polynomial used in Bluetooth communication protocols",
    },
    EnumVariant {
        value: "CRC_8_CDMA2000",
        description: "8-bit CRC used in CDMA2000 cellular communication standard",
    },
    EnumVariant {
        value: "CRC_8_DARC",
        description: "8-bit CRC used in DARC (Digital Audio Radio Channel) communication",
    },
    EnumVariant {
        value: "CRC_8_DVB_S2",
        description: "8-bit CRC used in DVB-S2 (Digital Video Broadcasting Satellite Second Generation)",
    },
    EnumVariant {
        value: "CRC_8_GSM_A",
        description: "8-bit CRC variant A used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_8_GSM_B",
        description: "8-bit CRC variant B used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_8_HITAG",
        description: "8-bit CRC used in Hitag RFID and transponder systems",
    },
    EnumVariant {
        value: "CRC_8_I_432_1",
        description: "8-bit CRC specified in IEEE 1432.1 standard",
    },
    EnumVariant {
        value: "CRC_8_I_CODE",
        description: "8-bit CRC used in I-CODE RFID systems",
    },
    EnumVariant {
        value: "CRC_8_LTE",
        description: "8-bit CRC used in LTE (Long-Term Evolution) cellular networks",
    },
    EnumVariant {
        value: "CRC_8_MAXIM_DOW",
        description: "8-bit CRC used by Maxim/Dallas Semiconductor for 1-Wire and iButton devices",
    },
    EnumVariant {
        value: "CRC_8_MIFARE_MAD",
        description: "8-bit CRC used in MIFARE MAD (Multiple Application Directory) protocol",
    },
    EnumVariant {
        value: "CRC_8_NRSC_5",
        description: "8-bit CRC used in NRSC-5 digital radio broadcasting standard",
    },
    EnumVariant {
        value: "CRC_8_OPENSAFETY",
        description: "8-bit CRC used in OpenSAFETY industrial communication protocol",
    },
    EnumVariant {
        value: "CRC_8_ROHC",
        description: "8-bit CRC used in Robust Header Compression (ROHC) protocol",
    },
    EnumVariant {
        value: "CRC_8_SAE_J1850",
        description: "8-bit CRC used in SAE J1850 automotive communication protocol",
    },
    EnumVariant {
        value: "CRC_8_SMBUS",
        description: "8-bit CRC used in System Management Bus (SMBus) communication",
    },
    EnumVariant {
        value: "CRC_8_TECH_3250",
        description: "8-bit CRC used in SMPTE (Society of Motion Picture and Television Engineers) standard",
    },
    EnumVariant {
        value: "CRC_8_WCDMA",
        description: "8-bit CRC used in WCDMA (Wideband Code Division Multiple Access) networks",
    },
    EnumVariant {
        value: "CRC_10_ATM",
        description: "10-bit CRC used in ATM (Asynchronous Transfer Mode) cell headers",
    },
    EnumVariant {
        value: "CRC_10_CDMA2000",
        description: "10-bit CRC used in CDMA2000 cellular communication standard",
    },
    EnumVariant {
        value: "CRC_10_GSM",
        description: "10-bit CRC variant used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_11_FLEXRAY",
        description: "11-bit CRC used in FlexRay automotive communication protocol",
    },
    EnumVariant {
        value: "CRC_11_UMTS",
        description: "11-bit CRC used in UMTS (Universal Mobile Telecommunications System)",
    },
    EnumVariant {
        value: "CRC_12_CDMA2000",
        description: "12-bit CRC used in CDMA2000 cellular communication standard",
    },
    EnumVariant {
        value: "CRC_12_DECT",
        description: "12-bit CRC used in DECT (Digital Enhanced Cordless Telecommunications) standards",
    },
    EnumVariant {
        value: "CRC_12_GSM",
        description: "12-bit CRC variant used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_12_UMTS",
        description: "12-bit CRC used in UMTS (Universal Mobile Telecommunications System)",
    },
    EnumVariant {
        value: "CRC_13_BBC",
        description: "13-bit CRC used in BBC (British Broadcasting Corporation) digital transmission",
    },
    EnumVariant {
        value: "CRC_14_DARC",
        description: "14-bit CRC used in DARC (Digital Audio Radio Channel) communication",
    },
    EnumVariant {
        value: "CRC_14_GSM",
        description: "14-bit CRC variant used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_15_CAN",
        description: "15-bit CRC used in CAN (Controller Area Network) automotive communication",
    },
    EnumVariant {
        value: "CRC_15_MPT1327",
        description: "15-bit CRC used in MPT 1327 radio trunking system",
    },
    EnumVariant {
        value: "CRC_16_ARC",
        description: "16-bit CRC used in ARC (Adaptive Routing Code) communication",
    },
    EnumVariant {
        value: "CRC_16_CDMA2000",
        description: "16-bit CRC used in CDMA2000 cellular communication standard",
    },
    EnumVariant {
        value: "CRC_16_CMS",
        description: "16-bit CRC used in Content Management Systems for data integrity",
    },
    EnumVariant {
        value: "CRC_16_DDS_110",
        description: "16-bit CRC used in DDS (Digital Data Storage) standard",
    },
    EnumVariant {
        value: "CRC_16_DECT_R",
        description: "16-bit CRC variant R used in DECT communication",
    },
    EnumVariant {
        value: "CRC_16_DECT_X",
        description: "16-bit CRC variant X used in DECT communication",
    },
    EnumVariant {
        value: "CRC_16_DNP",
        description: "16-bit CRC used in DNP3 (Distributed Network Protocol) for utilities",
    },
    EnumVariant {
        value: "CRC_16_EN_13757",
        description: "16-bit CRC specified in EN 13757 for meter communication",
    },
    EnumVariant {
        value: "CRC_16_GENIBUS",
        description: "16-bit CRC used in GENIBUS communication protocol",
    },
    EnumVariant {
        value: "CRC_16_GSM",
        description: "16-bit CRC variant used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_16_IBM_3740",
        description: "16-bit CRC used in IBM 3740 data integrity checks",
    },
    EnumVariant {
        value: "CRC_16_IBM_SDLC",
        description: "16-bit CRC used in IBM SDLC (Synchronous Data Link Control)",
    },
    EnumVariant {
        value: "CRC_16_ISO_IEC_14443_3_A",
        description: "16-bit CRC used in ISO/IEC 14443-3 Type A contactless smart cards",
    },
    EnumVariant {
        value: "CRC_16_KERMIT",
        description: "16-bit CRC used in Kermit file transfer protocol",
    },
    EnumVariant {
        value: "CRC_16_LJ1200",
        description: "16-bit CRC used in LJ1200 communication system",
    },
    EnumVariant {
        value: "CRC_16_M17",
        description: "16-bit CRC used in M17 digital radio communication",
    },
    EnumVariant {
        value: "CRC_16_MAXIM_DOW",
        description: "16-bit CRC used by Maxim/Dallas Semiconductor for data integrity",
    },
    EnumVariant {
        value: "CRC_16_MCRF4XX",
        description: "16-bit CRC used in MCRF4XX RFID systems",
    },
    EnumVariant {
        value: "CRC_16_MODBUS",
        description: "16-bit CRC used in Modbus communication protocol for error detection",
    },
    EnumVariant {
        value: "CRC_16_NRSC_5",
        description: "16-bit CRC used in NRSC-5 digital radio broadcasting standard",
    },
    EnumVariant {
        value: "CRC_16_OPENSAFETY_A",
        description: "16-bit CRC variant A in OpenSAFETY industrial communication",
    },
    EnumVariant {
        value: "CRC_16_OPENSAFETY_B",
        description: "16-bit CRC variant B in OpenSAFETY industrial communication",
    },
    EnumVariant {
        value: "CRC_16_PROFIBUS",
        description: "16-bit CRC used in PROFIBUS industrial communication protocol",
    },
    EnumVariant {
        value: "CRC_16_RIELLO",
        description: "16-bit CRC used in Riello UPS communication",
    },
    EnumVariant {
        value: "CRC_16_SPI_FUJITSU",
        description: "16-bit CRC used in Fujitsu SPI (Serial Peripheral Interface) communication",
    },
    EnumVariant {
        value: "CRC_16_T10_DIF",
        description: "16-bit CRC used in T10 DIF (Data Integrity Field) standard",
    },
    EnumVariant {
        value: "CRC_16_TELEDISK",
        description: "16-bit CRC used in Teledisk disk image format",
    },
    EnumVariant {
        value: "CRC_16_TMS37157",
        description: "16-bit CRC used in TMS37157 microcontroller communication",
    },
    EnumVariant {
        value: "CRC_16_UMTS",
        description: "16-bit CRC used in UMTS (Universal Mobile Telecommunications System)",
    },
    EnumVariant {
        value: "CRC_16_USB",
        description: "16-bit CRC used in USB communication for error detection",
    },
    EnumVariant {
        value: "CRC_16_XMODEM",
        description: "16-bit CRC used in XMODEM file transfer protocol",
    },
    EnumVariant {
        value: "CRC_17_CAN_FD",
        description: "17-bit CRC used in CAN FD (Flexible Data-Rate) automotive communication protocol",
    },
    EnumVariant {
        value: "CRC_21_CAN_FD",
        description: "21-bit CRC variant used in CAN FD (Flexible Data-Rate) automotive communication",
    },
    EnumVariant {
        value: "CRC_24_BLE",
        description: "24-bit CRC used in Bluetooth Low Energy (BLE) packet error checking",
    },
    EnumVariant {
        value: "CRC_24_FLEXRAY_A",
        description: "24-bit CRC variant A used in FlexRay automotive communication protocol",
    },
    EnumVariant {
        value: "CRC_24_FLEXRAY_B",
        description: "24-bit CRC variant B used in FlexRay automotive communication protocol",
    },
    EnumVariant {
        value: "CRC_24_INTERLAKEN",
        description: "24-bit CRC used in Interlaken high-speed serial communication protocol",
    },
    EnumVariant {
        value: "CRC_24_LTE_A",
        description: "24-bit CRC variant A used in LTE (Long-Term Evolution) cellular networks",
    },
    EnumVariant {
        value: "CRC_24_LTE_B",
        description: "24-bit CRC variant B used in LTE (Long-Term Evolution) cellular networks",
    },
    EnumVariant {
        value: "CRC_24_OPENPGP",
        description: "24-bit CRC used in OpenPGP (Pretty Good Privacy) for data integrity",
    },
    EnumVariant {
        value: "CRC_24_OS_9",
        description: "24-bit CRC used in OS-9 operating system for error detection",
    },
    EnumVariant {
        value: "CRC_30_CDMA",
        description: "30-bit CRC used in CDMA (Code Division Multiple Access) communication standard",
    },
    EnumVariant {
        value: "CRC_31_PHILIPS",
        description: "31-bit CRC used in Philips communication protocols",
    },
    EnumVariant {
        value: "CRC_32_AIXM",
        description: "32-bit CRC used in Aeronautical Information Exchange Model (AIXM)",
    },
    EnumVariant {
        value: "CRC_32_AUTOSAR",
        description: "32-bit CRC used in AUTOSAR (Automotive Open System Architecture) standard",
    },
    EnumVariant {
        value: "CRC_32_BASE91_D",
        description: "32-bit CRC variant used in Base91 data encoding",
    },
    EnumVariant {
        value: "CRC_32_BZIP2",
        description: "32-bit CRC used in bzip2 compression algorithm",
    },
    EnumVariant {
        value: "CRC_32_CD_ROM_EDC",
        description: "32-bit CRC used for Error Detection Code in CD-ROM systems",
    },
    EnumVariant {
        value: "CRC_32_CKSUM",
        description: "32-bit CRC used in UNIX cksum command for file integrity",
    },
    EnumVariant {
        value: "CRC_32_ISCSI",
        description: "32-bit CRC used in iSCSI (Internet Small Computer Systems Interface)",
    },
    EnumVariant {
        value: "CRC_32_ISO_HDLC",
        description: "32-bit CRC used in ISO HDLC (High-Level Data Link Control)",
    },
    EnumVariant {
        value: "CRC_32_JAMCRC",
        description: "32-bit CRC variant used in JAM error detection",
    },
    EnumVariant {
        value: "CRC_32_MEF",
        description: "32-bit CRC used in Metro Ethernet Forum (MEF) standards",
    },
    EnumVariant {
        value: "CRC_32_MPEG_2",
        description: "32-bit CRC used in MPEG-2 transport streams for error detection",
    },
    EnumVariant {
        value: "CRC_32_XFER",
        description: "32-bit CRC used in data transfer protocols",
    },
    EnumVariant {
        value: "CRC_40_GSM",
        description: "40-bit CRC variant used in GSM telecommunications",
    },
    EnumVariant {
        value: "CRC_64_ECMA_182",
        description: "64-bit CRC specified in ECMA-182 standard",
    },
    EnumVariant {
        value: "CRC_64_GO_ISO",
        description: "64-bit CRC used in Go programming language and ISO standards",
    },
    EnumVariant {
        value: "CRC_64_MS",
        description: "64-bit CRC variant used in Microsoft systems",
    },
    EnumVariant {
        value: "CRC_64_REDIS",
        description: "64-bit CRC used in Redis key-value data store",
    },
    EnumVariant {
        value: "CRC_64_WE",
        description: "64-bit CRC variant for wide-area error detection",
    },
    EnumVariant {
        value: "CRC_64_XZ",
        description: "64-bit CRC used in the XZ compression format for integrity verification",
    },
    EnumVariant {
        value: "CRC_82_DARC",
        description: "82-bit CRC used in DARC (Digital Audio Radio Channel) communication",
    },
];

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The string to calculate the checksum for.",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "algorithm",
            kind: kind::BYTES,
            required: false,
            description: "The CRC algorithm to use.",
            default: Some(&DEFAULT_ALGORITHM),
            enum_variants: Some(ALGORITHM_ENUM),
        },
    ]
});

#[allow(clippy::too_many_lines)]
fn crc(value: Value, algorithm: &str) -> Resolved {
    let value = value.try_bytes()?;

    let checksum = match algorithm {
        "CRC_3_GSM" => CrcInstance::<u8>::new(&crc::CRC_3_GSM)
            .checksum(&value)
            .to_string(),
        "CRC_3_ROHC" => CrcInstance::<u8>::new(&crc::CRC_3_ROHC)
            .checksum(&value)
            .to_string(),
        "CRC_4_G_704" => CrcInstance::<u8>::new(&crc::CRC_4_G_704)
            .checksum(&value)
            .to_string(),
        "CRC_4_INTERLAKEN" => CrcInstance::<u8>::new(&crc::CRC_4_INTERLAKEN)
            .checksum(&value)
            .to_string(),
        "CRC_5_EPC_C1G2" => CrcInstance::<u8>::new(&crc::CRC_5_EPC_C1G2)
            .checksum(&value)
            .to_string(),
        "CRC_5_G_704" => CrcInstance::<u8>::new(&crc::CRC_5_G_704)
            .checksum(&value)
            .to_string(),
        "CRC_5_USB" => CrcInstance::<u8>::new(&crc::CRC_5_USB)
            .checksum(&value)
            .to_string(),
        "CRC_6_CDMA2000_A" => CrcInstance::<u8>::new(&crc::CRC_6_CDMA2000_A)
            .checksum(&value)
            .to_string(),
        "CRC_6_CDMA2000_B" => CrcInstance::<u8>::new(&crc::CRC_6_CDMA2000_B)
            .checksum(&value)
            .to_string(),
        "CRC_6_DARC" => CrcInstance::<u8>::new(&crc::CRC_6_DARC)
            .checksum(&value)
            .to_string(),
        "CRC_6_GSM" => CrcInstance::<u8>::new(&crc::CRC_6_GSM)
            .checksum(&value)
            .to_string(),
        "CRC_6_G_704" => CrcInstance::<u8>::new(&crc::CRC_6_G_704)
            .checksum(&value)
            .to_string(),
        "CRC_7_MMC" => CrcInstance::<u8>::new(&crc::CRC_7_MMC)
            .checksum(&value)
            .to_string(),
        "CRC_7_ROHC" => CrcInstance::<u8>::new(&crc::CRC_7_ROHC)
            .checksum(&value)
            .to_string(),
        "CRC_7_UMTS" => CrcInstance::<u8>::new(&crc::CRC_7_UMTS)
            .checksum(&value)
            .to_string(),
        "CRC_8_AUTOSAR" => CrcInstance::<u8>::new(&crc::CRC_8_AUTOSAR)
            .checksum(&value)
            .to_string(),
        "CRC_8_BLUETOOTH" => CrcInstance::<u8>::new(&crc::CRC_8_BLUETOOTH)
            .checksum(&value)
            .to_string(),
        "CRC_8_CDMA2000" => CrcInstance::<u8>::new(&crc::CRC_8_CDMA2000)
            .checksum(&value)
            .to_string(),
        "CRC_8_DARC" => CrcInstance::<u8>::new(&crc::CRC_8_DARC)
            .checksum(&value)
            .to_string(),
        "CRC_8_DVB_S2" => CrcInstance::<u8>::new(&crc::CRC_8_DVB_S2)
            .checksum(&value)
            .to_string(),
        "CRC_8_GSM_A" => CrcInstance::<u8>::new(&crc::CRC_8_GSM_A)
            .checksum(&value)
            .to_string(),
        "CRC_8_GSM_B" => CrcInstance::<u8>::new(&crc::CRC_8_GSM_B)
            .checksum(&value)
            .to_string(),
        "CRC_8_HITAG" => CrcInstance::<u8>::new(&crc::CRC_8_HITAG)
            .checksum(&value)
            .to_string(),
        "CRC_8_I_432_1" => CrcInstance::<u8>::new(&crc::CRC_8_I_432_1)
            .checksum(&value)
            .to_string(),
        "CRC_8_I_CODE" => CrcInstance::<u8>::new(&crc::CRC_8_I_CODE)
            .checksum(&value)
            .to_string(),
        "CRC_8_LTE" => CrcInstance::<u8>::new(&crc::CRC_8_LTE)
            .checksum(&value)
            .to_string(),
        "CRC_8_MAXIM_DOW" => CrcInstance::<u8>::new(&crc::CRC_8_MAXIM_DOW)
            .checksum(&value)
            .to_string(),
        "CRC_8_MIFARE_MAD" => CrcInstance::<u8>::new(&crc::CRC_8_MIFARE_MAD)
            .checksum(&value)
            .to_string(),
        "CRC_8_NRSC_5" => CrcInstance::<u8>::new(&crc::CRC_8_NRSC_5)
            .checksum(&value)
            .to_string(),
        "CRC_8_OPENSAFETY" => CrcInstance::<u8>::new(&crc::CRC_8_OPENSAFETY)
            .checksum(&value)
            .to_string(),
        "CRC_8_ROHC" => CrcInstance::<u8>::new(&crc::CRC_8_ROHC)
            .checksum(&value)
            .to_string(),
        "CRC_8_SAE_J1850" => CrcInstance::<u8>::new(&crc::CRC_8_SAE_J1850)
            .checksum(&value)
            .to_string(),
        "CRC_8_SMBUS" => CrcInstance::<u8>::new(&crc::CRC_8_SMBUS)
            .checksum(&value)
            .to_string(),
        "CRC_8_TECH_3250" => CrcInstance::<u8>::new(&crc::CRC_8_TECH_3250)
            .checksum(&value)
            .to_string(),
        "CRC_8_WCDMA" => CrcInstance::<u8>::new(&crc::CRC_8_WCDMA)
            .checksum(&value)
            .to_string(),
        "CRC_10_ATM" => CrcInstance::<u16>::new(&crc::CRC_10_ATM)
            .checksum(&value)
            .to_string(),
        "CRC_10_CDMA2000" => CrcInstance::<u16>::new(&crc::CRC_10_CDMA2000)
            .checksum(&value)
            .to_string(),
        "CRC_10_GSM" => CrcInstance::<u16>::new(&crc::CRC_10_GSM)
            .checksum(&value)
            .to_string(),
        "CRC_11_FLEXRAY" => CrcInstance::<u16>::new(&crc::CRC_11_FLEXRAY)
            .checksum(&value)
            .to_string(),
        "CRC_11_UMTS" => CrcInstance::<u16>::new(&crc::CRC_11_UMTS)
            .checksum(&value)
            .to_string(),
        "CRC_12_CDMA2000" => CrcInstance::<u16>::new(&crc::CRC_12_CDMA2000)
            .checksum(&value)
            .to_string(),
        "CRC_12_DECT" => CrcInstance::<u16>::new(&crc::CRC_12_DECT)
            .checksum(&value)
            .to_string(),
        "CRC_12_GSM" => CrcInstance::<u16>::new(&crc::CRC_12_GSM)
            .checksum(&value)
            .to_string(),
        "CRC_12_UMTS" => CrcInstance::<u16>::new(&crc::CRC_12_UMTS)
            .checksum(&value)
            .to_string(),
        "CRC_13_BBC" => CrcInstance::<u16>::new(&crc::CRC_13_BBC)
            .checksum(&value)
            .to_string(),
        "CRC_14_DARC" => CrcInstance::<u16>::new(&crc::CRC_14_DARC)
            .checksum(&value)
            .to_string(),
        "CRC_14_GSM" => CrcInstance::<u16>::new(&crc::CRC_14_GSM)
            .checksum(&value)
            .to_string(),
        "CRC_15_CAN" => CrcInstance::<u16>::new(&crc::CRC_15_CAN)
            .checksum(&value)
            .to_string(),
        "CRC_15_MPT1327" => CrcInstance::<u16>::new(&crc::CRC_15_MPT1327)
            .checksum(&value)
            .to_string(),
        "CRC_16_ARC" => CrcInstance::<u16>::new(&crc::CRC_16_ARC)
            .checksum(&value)
            .to_string(),
        "CRC_16_CDMA2000" => CrcInstance::<u16>::new(&crc::CRC_16_CDMA2000)
            .checksum(&value)
            .to_string(),
        "CRC_16_CMS" => CrcInstance::<u16>::new(&crc::CRC_16_CMS)
            .checksum(&value)
            .to_string(),
        "CRC_16_DDS_110" => CrcInstance::<u16>::new(&crc::CRC_16_DDS_110)
            .checksum(&value)
            .to_string(),
        "CRC_16_DECT_R" => CrcInstance::<u16>::new(&crc::CRC_16_DECT_R)
            .checksum(&value)
            .to_string(),
        "CRC_16_DECT_X" => CrcInstance::<u16>::new(&crc::CRC_16_DECT_X)
            .checksum(&value)
            .to_string(),
        "CRC_16_DNP" => CrcInstance::<u16>::new(&crc::CRC_16_DNP)
            .checksum(&value)
            .to_string(),
        "CRC_16_EN_13757" => CrcInstance::<u16>::new(&crc::CRC_16_EN_13757)
            .checksum(&value)
            .to_string(),
        "CRC_16_GENIBUS" => CrcInstance::<u16>::new(&crc::CRC_16_GENIBUS)
            .checksum(&value)
            .to_string(),
        "CRC_16_GSM" => CrcInstance::<u16>::new(&crc::CRC_16_GSM)
            .checksum(&value)
            .to_string(),
        "CRC_16_IBM_3740" => CrcInstance::<u16>::new(&crc::CRC_16_IBM_3740)
            .checksum(&value)
            .to_string(),
        "CRC_16_IBM_SDLC" => CrcInstance::<u16>::new(&crc::CRC_16_IBM_SDLC)
            .checksum(&value)
            .to_string(),
        "CRC_16_ISO_IEC_14443_3_A" => CrcInstance::<u16>::new(&crc::CRC_16_ISO_IEC_14443_3_A)
            .checksum(&value)
            .to_string(),
        "CRC_16_KERMIT" => CrcInstance::<u16>::new(&crc::CRC_16_KERMIT)
            .checksum(&value)
            .to_string(),
        "CRC_16_LJ1200" => CrcInstance::<u16>::new(&crc::CRC_16_LJ1200)
            .checksum(&value)
            .to_string(),
        "CRC_16_M17" => CrcInstance::<u16>::new(&crc::CRC_16_M17)
            .checksum(&value)
            .to_string(),
        "CRC_16_MAXIM_DOW" => CrcInstance::<u16>::new(&crc::CRC_16_MAXIM_DOW)
            .checksum(&value)
            .to_string(),
        "CRC_16_MCRF4XX" => CrcInstance::<u16>::new(&crc::CRC_16_MCRF4XX)
            .checksum(&value)
            .to_string(),
        "CRC_16_MODBUS" => CrcInstance::<u16>::new(&crc::CRC_16_MODBUS)
            .checksum(&value)
            .to_string(),
        "CRC_16_NRSC_5" => CrcInstance::<u16>::new(&crc::CRC_16_NRSC_5)
            .checksum(&value)
            .to_string(),
        "CRC_16_OPENSAFETY_A" => CrcInstance::<u16>::new(&crc::CRC_16_OPENSAFETY_A)
            .checksum(&value)
            .to_string(),
        "CRC_16_OPENSAFETY_B" => CrcInstance::<u16>::new(&crc::CRC_16_OPENSAFETY_B)
            .checksum(&value)
            .to_string(),
        "CRC_16_PROFIBUS" => CrcInstance::<u16>::new(&crc::CRC_16_PROFIBUS)
            .checksum(&value)
            .to_string(),
        "CRC_16_RIELLO" => CrcInstance::<u16>::new(&crc::CRC_16_RIELLO)
            .checksum(&value)
            .to_string(),
        "CRC_16_SPI_FUJITSU" => CrcInstance::<u16>::new(&crc::CRC_16_SPI_FUJITSU)
            .checksum(&value)
            .to_string(),
        "CRC_16_T10_DIF" => CrcInstance::<u16>::new(&crc::CRC_16_T10_DIF)
            .checksum(&value)
            .to_string(),
        "CRC_16_TELEDISK" => CrcInstance::<u16>::new(&crc::CRC_16_TELEDISK)
            .checksum(&value)
            .to_string(),
        "CRC_16_TMS37157" => CrcInstance::<u16>::new(&crc::CRC_16_TMS37157)
            .checksum(&value)
            .to_string(),
        "CRC_16_UMTS" => CrcInstance::<u16>::new(&crc::CRC_16_UMTS)
            .checksum(&value)
            .to_string(),
        "CRC_16_USB" => CrcInstance::<u16>::new(&crc::CRC_16_USB)
            .checksum(&value)
            .to_string(),
        "CRC_16_XMODEM" => CrcInstance::<u16>::new(&crc::CRC_16_XMODEM)
            .checksum(&value)
            .to_string(),
        "CRC_17_CAN_FD" => CrcInstance::<u32>::new(&crc::CRC_17_CAN_FD)
            .checksum(&value)
            .to_string(),
        "CRC_21_CAN_FD" => CrcInstance::<u32>::new(&crc::CRC_21_CAN_FD)
            .checksum(&value)
            .to_string(),
        "CRC_24_BLE" => CrcInstance::<u32>::new(&crc::CRC_24_BLE)
            .checksum(&value)
            .to_string(),
        "CRC_24_FLEXRAY_A" => CrcInstance::<u32>::new(&crc::CRC_24_FLEXRAY_A)
            .checksum(&value)
            .to_string(),
        "CRC_24_FLEXRAY_B" => CrcInstance::<u32>::new(&crc::CRC_24_FLEXRAY_B)
            .checksum(&value)
            .to_string(),
        "CRC_24_INTERLAKEN" => CrcInstance::<u32>::new(&crc::CRC_24_INTERLAKEN)
            .checksum(&value)
            .to_string(),
        "CRC_24_LTE_A" => CrcInstance::<u32>::new(&crc::CRC_24_LTE_A)
            .checksum(&value)
            .to_string(),
        "CRC_24_LTE_B" => CrcInstance::<u32>::new(&crc::CRC_24_LTE_B)
            .checksum(&value)
            .to_string(),
        "CRC_24_OPENPGP" => CrcInstance::<u32>::new(&crc::CRC_24_OPENPGP)
            .checksum(&value)
            .to_string(),
        "CRC_24_OS_9" => CrcInstance::<u32>::new(&crc::CRC_24_OS_9)
            .checksum(&value)
            .to_string(),
        "CRC_30_CDMA" => CrcInstance::<u32>::new(&crc::CRC_30_CDMA)
            .checksum(&value)
            .to_string(),
        "CRC_31_PHILIPS" => CrcInstance::<u32>::new(&crc::CRC_31_PHILIPS)
            .checksum(&value)
            .to_string(),
        "CRC_32_AIXM" => CrcInstance::<u32>::new(&crc::CRC_32_AIXM)
            .checksum(&value)
            .to_string(),
        "CRC_32_AUTOSAR" => CrcInstance::<u32>::new(&crc::CRC_32_AUTOSAR)
            .checksum(&value)
            .to_string(),
        "CRC_32_BASE91_D" => CrcInstance::<u32>::new(&crc::CRC_32_BASE91_D)
            .checksum(&value)
            .to_string(),
        "CRC_32_BZIP2" => CrcInstance::<u32>::new(&crc::CRC_32_BZIP2)
            .checksum(&value)
            .to_string(),
        "CRC_32_CD_ROM_EDC" => CrcInstance::<u32>::new(&crc::CRC_32_CD_ROM_EDC)
            .checksum(&value)
            .to_string(),
        "CRC_32_CKSUM" => CrcInstance::<u32>::new(&crc::CRC_32_CKSUM)
            .checksum(&value)
            .to_string(),
        "CRC_32_ISCSI" => CrcInstance::<u32>::new(&crc::CRC_32_ISCSI)
            .checksum(&value)
            .to_string(),
        "CRC_32_ISO_HDLC" => CrcInstance::<u32>::new(&crc::CRC_32_ISO_HDLC)
            .checksum(&value)
            .to_string(),
        "CRC_32_JAMCRC" => CrcInstance::<u32>::new(&crc::CRC_32_JAMCRC)
            .checksum(&value)
            .to_string(),
        "CRC_32_MEF" => CrcInstance::<u32>::new(&crc::CRC_32_MEF)
            .checksum(&value)
            .to_string(),
        "CRC_32_MPEG_2" => CrcInstance::<u32>::new(&crc::CRC_32_MPEG_2)
            .checksum(&value)
            .to_string(),
        "CRC_32_XFER" => CrcInstance::<u32>::new(&crc::CRC_32_XFER)
            .checksum(&value)
            .to_string(),
        "CRC_40_GSM" => CrcInstance::<u64>::new(&crc::CRC_40_GSM)
            .checksum(&value)
            .to_string(),
        "CRC_64_ECMA_182" => CrcInstance::<u64>::new(&crc::CRC_64_ECMA_182)
            .checksum(&value)
            .to_string(),
        "CRC_64_GO_ISO" => CrcInstance::<u64>::new(&crc::CRC_64_GO_ISO)
            .checksum(&value)
            .to_string(),
        "CRC_64_MS" => CrcInstance::<u64>::new(&crc::CRC_64_MS)
            .checksum(&value)
            .to_string(),
        "CRC_64_REDIS" => CrcInstance::<u64>::new(&crc::CRC_64_REDIS)
            .checksum(&value)
            .to_string(),
        "CRC_64_WE" => CrcInstance::<u64>::new(&crc::CRC_64_WE)
            .checksum(&value)
            .to_string(),
        "CRC_64_XZ" => CrcInstance::<u64>::new(&crc::CRC_64_XZ)
            .checksum(&value)
            .to_string(),
        "CRC_82_DARC" => CrcInstance::<u128>::new(&crc::CRC_82_DARC)
            .checksum(&value)
            .to_string(),
        _ => return Err(format!("Invalid CRC algorithm: {algorithm}").into()),
    };

    Ok(checksum.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Crc;

impl Function for Crc {
    fn identifier(&self) -> &'static str {
        "crc"
    }

    fn usage(&self) -> &'static str {
        indoc! {
            "Calculates a CRC of the `value`.The CRC `algorithm` used can be optionally specified.

            This function is infallible if either the default `algorithm` value or a recognized-valid compile-time `algorithm` string literal is used. Otherwise, it is fallible."
        }
    }

    fn category(&self) -> &'static str {
        Category::Checksum.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`value` is not a string.",
            "`algorithm` is not a supported algorithm.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Create CRC checksum using the default algorithm",
                source: r#"crc("foo")"#,
                result: Ok(r#""2356372769""#),
            },
            example! {
                title: "Create CRC checksum using the CRC_32_CKSUM algorithm",
                source: r#"crc("foo", algorithm: "CRC_32_CKSUM")"#,
                result: Ok(r#""4271552933""#),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let algorithm = arguments.optional("algorithm");

        Ok(CrcFn { value, algorithm }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct CrcFn {
    value: Box<dyn Expression>,
    algorithm: Option<Box<dyn Expression>>,
}

impl FunctionExpression for CrcFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let algorithm = self
            .algorithm
            .map_resolve_with_default(ctx, || DEFAULT_ALGORITHM.clone())?;

        let algorithm = algorithm.try_bytes_utf8_lossy()?.as_ref().to_uppercase();
        crc(value, &algorithm)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let algorithm = self.algorithm.as_ref();
        let valid_static_algo = algorithm.is_none()
            || algorithm
                .and_then(|algorithm| algorithm.resolve_constant(state))
                .and_then(|algorithm| algorithm.try_bytes_utf8_lossy().map(|s| s.to_string()).ok())
                .is_some_and(|algorithm| {
                    VALID_ALGORITHMS.contains(&algorithm.to_uppercase().as_str())
                });

        if valid_static_algo {
            TypeDef::bytes().infallible()
        } else {
            TypeDef::bytes().fallible()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        crc => Crc;

        crc_default {
            args: func_args![value: "foo"],
            want: Ok(value!(b"2356372769")),
            tdef: TypeDef::bytes().infallible(),
        }

        crc_crc8 {
            args: func_args![value: "foo", algorithm: "CRC_8_MAXIM_DOW"],
            want: Ok(value!(b"18")),
            tdef: TypeDef::bytes().infallible(),
        }

        crc_crc32 {
            args: func_args![value: "foo", algorithm: "CRC_32_CKSUM"],
            want: Ok(value!(b"4271552933")),
            tdef: TypeDef::bytes().infallible(),
        }

        crc_crc64 {
            args: func_args![value: "foo", algorithm: "CRC_64_REDIS"],
            want: Ok(value!(b"12626267673720558670")),
            tdef: TypeDef::bytes().infallible(),
        }

        crc_unknown {
            args: func_args![value: "foo", algorithm: "CRC_UNKNOWN"],
            want: Err("Invalid CRC algorithm: CRC_UNKNOWN"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
