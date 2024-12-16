use crate::compiler::prelude::*;
use crate::value;
use crc::Crc as CrcInstance;

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

fn crc(value: Value, algorithm: Value) -> Resolved {
    let value = value.try_bytes()?;
    let algorithm = algorithm.try_bytes_utf8_lossy()?.as_ref().to_uppercase();

    let checksum = match algorithm.as_str() {
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

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "algorithm",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "default CRC_32_ISO_HDLC",
                source: r#"crc("foobar")"#,
                result: Ok(r#""2666930069""#),
            },
            Example {
                title: "CRC_8_MAXIM_DOW",
                source: r#"crc("foobar", algorithm: "CRC_8_MAXIM_DOW")"#,
                result: Ok(r#""53""#),
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
        let algorithm = match &self.algorithm {
            Some(algorithm) => algorithm.resolve(ctx)?,
            None => value!("CRC_32_ISO_HDLC"),
        };

        crc(value, algorithm)
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
