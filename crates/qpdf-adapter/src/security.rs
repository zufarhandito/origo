//! Offline PDF password protection (AES-256) and unlocking via bundled QPDF 12.4.2.
//! Passwords are passed strictly through ephemeral `--job-json-file` manifests that are
//! deleted immediately after QPDF exits; they are never placed on the process command line.
use pdf_application::export::{ExportControl, ExportError};
use pdf_platform::{StagedPdfOutput, run};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fmt,
    path::{Path, PathBuf},
    process::Command,
};

const MAX_PASSWORD_BYTES: usize = 128;
const MAX_SOURCE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityError {
    Dependency,
    SourceMissing,
    SourceTooLarge,
    AlreadyEncrypted,
    NotEncrypted,
    EmptyPassword,
    InvalidPassword,
    WrongPassword,
    DestinationExists,
    InvalidDestination,
    Cancelled,
    Validation,
    Io,
    Engine,
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Dependency => "QPDF 12.4.2 tidak ditemukan atau versinya tidak cocok.",
            Self::SourceMissing => "File PDF sumber tidak ditemukan atau tidak bisa dibaca.",
            Self::SourceTooLarge => "Ukuran file PDF melebihi batas 128 MiB.",
            Self::AlreadyEncrypted => {
                "PDF ini sudah dilindungi password. Buka kunci terlebih dahulu jika ingin mengubah password."
            }
            Self::NotEncrypted => "PDF ini tidak terenkripsi dan tidak memerlukan password.",
            Self::EmptyPassword => "Masukkan password minimal 1 karakter.",
            Self::InvalidPassword => {
                "Password maksimal 128 byte dan tidak boleh berisi karakter baris baru."
            }
            Self::WrongPassword => "Password tidak cocok. Periksa kembali password PDF.",
            Self::DestinationExists => {
                "File tujuan sudah ada. Pilih nama file lain agar file lama tidak tertimpa."
            }
            Self::InvalidDestination => "Lokasi atau nama file tujuan tidak valid (.pdf).",
            Self::Cancelled => "Proses dibatalkan.",
            Self::Validation => "Hasil PDF tidak lolos pemeriksaan integritas.",
            Self::Io => "Gagal membaca atau menulis file sementara.",
            Self::Engine => "Gagal memproses keamanan PDF.",
        };
        f.write_str(message)
    }
}

impl std::error::Error for SecurityError {}

impl From<ExportError> for SecurityError {
    fn from(error: ExportError) -> Self {
        match error {
            ExportError::Cancelled => Self::Cancelled,
            ExportError::DestinationExists => Self::DestinationExists,
            ExportError::Destination | ExportError::InvalidPlan => Self::InvalidDestination,
            ExportError::Dependency => Self::Dependency,
            ExportError::Validation | ExportError::Structure => Self::Validation,
            ExportError::Io => Self::Io,
            _ => Self::Engine,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncryptPermissions {
    pub allow_print: bool,
    pub allow_modify: bool,
    pub allow_copy: bool,
}

impl Default for EncryptPermissions {
    fn default() -> Self {
        Self {
            allow_print: true,
            allow_modify: false,
            allow_copy: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncryptOptions {
    pub user_password: String,
    pub owner_password: String,
    pub permissions: EncryptPermissions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityReport {
    pub output: PathBuf,
    pub bytes: u64,
    pub encrypted: bool,
}

#[derive(Clone)]
pub struct QpdfSecurity {
    executable: PathBuf,
}

impl QpdfSecurity {
    pub fn new(sidecars: &Path) -> Self {
        Self {
            executable: sidecars.join("qpdf/bin/qpdf.exe"),
        }
    }

    fn invoke(
        &self,
        args: &[OsString],
        control: &ExportControl,
    ) -> Result<pdf_platform::ProcessOutput, SecurityError> {
        let mut command = Command::new(&self.executable);
        command.args(args);
        run(command, 65536, control).map_err(SecurityError::from)
    }

    fn verify_version(&self, control: &ExportControl) -> Result<(), SecurityError> {
        let out = self.invoke(&["--version".into()], control)?;
        if out.code != Some(0)
            || String::from_utf8_lossy(&out.stdout).lines().next() != Some("qpdf version 12.4.2")
        {
            return Err(SecurityError::Dependency);
        }
        Ok(())
    }

    fn run_ephemeral_job(
        &self,
        job: &Value,
        work: &Path,
        name: &str,
        control: &ExportControl,
    ) -> Result<Option<i32>, SecurityError> {
        control.checkpoint()?;
        let bytes = serde_json::to_vec(job).map_err(|_| SecurityError::Engine)?;
        let path = work.join(name);
        std::fs::write(&path, bytes).map_err(|_| SecurityError::Io)?;
        let mut arg = OsString::from("--job-json-file=");
        arg.push(&path);
        let result = self.invoke(&[arg], control);
        // Always overwrite/unlink job manifest containing passwords before returning.
        let _ = std::fs::write(&path, b"{}");
        let _ = std::fs::remove_file(&path);
        Ok(result?.code)
    }

    fn canonical_source(&self, source: &Path) -> Result<PathBuf, SecurityError> {
        let meta = std::fs::metadata(source).map_err(|_| SecurityError::SourceMissing)?;
        if !meta.is_file() {
            return Err(SecurityError::SourceMissing);
        }
        if meta.len() == 0 {
            return Err(SecurityError::Validation);
        }
        if meta.len() > MAX_SOURCE_BYTES {
            return Err(SecurityError::SourceTooLarge);
        }
        dunce::canonicalize(source).map_err(|_| SecurityError::SourceMissing)
    }

    pub fn is_encrypted(
        &self,
        source: &Path,
        control: &ExportControl,
    ) -> Result<bool, SecurityError> {
        self.verify_version(control)?;
        let source = self.canonical_source(source)?;
        let out = self.invoke(
            &["--is-encrypted".into(), source.as_os_str().to_owned()],
            control,
        )?;
        match out.code {
            Some(0) => Ok(true),
            Some(2) => Ok(false),
            _ => Err(SecurityError::Validation),
        }
    }

    pub fn encrypt_pdf(
        &self,
        source: &Path,
        destination: &Path,
        options: &EncryptOptions,
        control: &ExportControl,
    ) -> Result<SecurityReport, SecurityError> {
        validate_password(&options.user_password, false)?;
        validate_password(&options.owner_password, true)?;
        self.verify_version(control)?;
        let source = self.canonical_source(source)?;
        if self.is_encrypted(&source, control)? {
            return Err(SecurityError::AlreadyEncrypted);
        }
        let staged = StagedPdfOutput::stage(destination, control)?;
        let src_str = source.to_str().ok_or(SecurityError::SourceMissing)?;
        let out_str = staged
            .output_path()
            .to_str()
            .ok_or(SecurityError::InvalidDestination)?;
        let owner = if options.owner_password.is_empty() {
            options.user_password.as_str()
        } else {
            options.owner_password.as_str()
        };
        let encrypt_job = build_encrypt_job(
            src_str,
            out_str,
            &options.user_password,
            owner,
            options.permissions,
        );
        let code =
            self.run_ephemeral_job(&encrypt_job, staged.work_dir(), "encrypt.json", control)?;
        if !matches!(code, Some(0 | 3)) {
            return Err(SecurityError::Engine);
        }
        // Verify output is encrypted AND structurally valid when opened with user_password.
        let enc_check = self.invoke(
            &[
                "--is-encrypted".into(),
                staged.output_path().as_os_str().to_owned(),
            ],
            control,
        )?;
        if enc_check.code != Some(0) {
            return Err(SecurityError::Validation);
        }
        let verify_job = json!({
            "inputFile": out_str,
            "password": options.user_password,
            "check": ""
        });
        let verify_code =
            self.run_ephemeral_job(&verify_job, staged.work_dir(), "verify.json", control)?;
        if !matches!(verify_code, Some(0 | 3)) {
            return Err(SecurityError::Validation);
        }
        let (output, bytes) = staged.commit(control)?;
        Ok(SecurityReport {
            output,
            bytes,
            encrypted: true,
        })
    }

    pub fn decrypt_pdf(
        &self,
        source: &Path,
        destination: &Path,
        password: &str,
        control: &ExportControl,
    ) -> Result<SecurityReport, SecurityError> {
        validate_password(password, true)?;
        self.verify_version(control)?;
        let source = self.canonical_source(source)?;
        let staged = StagedPdfOutput::stage(destination, control)?;
        let src_str = source.to_str().ok_or(SecurityError::SourceMissing)?;
        let out_str = staged
            .output_path()
            .to_str()
            .ok_or(SecurityError::InvalidDestination)?;

        // Probe encryption & password validity deterministically via --requires-password:
        // 0 = wrong/missing password, 2 = not encrypted, 3 = encrypted & password accepted.
        let probe_job = json!({
            "inputFile": src_str,
            "password": password,
            "requiresPassword": ""
        });
        match self.run_ephemeral_job(&probe_job, staged.work_dir(), "probe.json", control)? {
            Some(2) => return Err(SecurityError::NotEncrypted),
            Some(0) => {
                return Err(if password.is_empty() {
                    SecurityError::EmptyPassword
                } else {
                    SecurityError::WrongPassword
                });
            }
            Some(3) => {}
            _ => return Err(SecurityError::Validation),
        }

        let decrypt_job = json!({
            "inputFile": src_str,
            "password": password,
            "decrypt": "",
            "outputFile": out_str
        });
        let code =
            self.run_ephemeral_job(&decrypt_job, staged.work_dir(), "decrypt.json", control)?;
        if !matches!(code, Some(0 | 3)) {
            return Err(SecurityError::Engine);
        }
        let enc_check = self.invoke(
            &[
                "--is-encrypted".into(),
                staged.output_path().as_os_str().to_owned(),
            ],
            control,
        )?;
        if enc_check.code != Some(2) {
            return Err(SecurityError::Validation);
        }
        let check = self.invoke(
            &[
                "--check".into(),
                staged.output_path().as_os_str().to_owned(),
            ],
            control,
        )?;
        if !matches!(check.code, Some(0 | 3)) {
            return Err(SecurityError::Validation);
        }
        let (output, bytes) = staged.commit(control)?;
        Ok(SecurityReport {
            output,
            bytes,
            encrypted: false,
        })
    }
}

fn validate_password(password: &str, allow_empty: bool) -> Result<(), SecurityError> {
    if !allow_empty && password.is_empty() {
        return Err(SecurityError::EmptyPassword);
    }
    if password.len() > MAX_PASSWORD_BYTES || password.chars().any(char::is_control) {
        return Err(SecurityError::InvalidPassword);
    }
    Ok(())
}

fn build_encrypt_job(
    input: &str,
    output: &str,
    user_password: &str,
    owner_password: &str,
    permissions: EncryptPermissions,
) -> Value {
    json!({
        "inputFile": input,
        "outputFile": output,
        "encrypt": {
            "userPassword": user_password,
            "ownerPassword": owner_password,
            "256bit": {
                "print": if permissions.allow_print { "full" } else { "none" },
                "modify": if permissions.allow_modify { "all" } else { "none" },
                "extract": if permissions.allow_copy { "y" } else { "n" }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn minimal_pdf() -> Vec<u8> {
        let mut pdf = Vec::from(b"%PDF-1.4\n".as_slice());
        let mut offsets = vec![0usize];
        let objects = [
            "1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
            "2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n",
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n",
        ];
        for obj in objects {
            offsets.push(pdf.len());
            pdf.extend_from_slice(obj.as_bytes());
        }
        let xref = pdf.len();
        pdf.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
        for off in &offsets[1..] {
            pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        pdf.extend_from_slice(
            format!("trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        pdf
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("origo-sec-{label}-{nonce}"));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn encrypt_aes256_and_decrypt_roundtrip_with_real_qpdf()
    -> Result<(), Box<dyn std::error::Error>> {
        let sidecars = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecars");
        let sec = QpdfSecurity::new(&sidecars);
        let control = ExportControl::default();
        let dir = temp_dir("roundtrip");
        let src = dir.join("plain.pdf");
        let locked = dir.join("locked.pdf");
        let unlocked = dir.join("unlocked.pdf");
        std::fs::write(&src, minimal_pdf())?;

        assert!(!sec.is_encrypted(&src, &control)?);
        // Decrypting a plain PDF returns NotEncrypted.
        assert_eq!(
            sec.decrypt_pdf(&src, &unlocked, "abc", &control),
            Err(SecurityError::NotEncrypted)
        );
        assert!(!unlocked.exists());

        // Encrypt with AES-256 and distinct user/owner passwords.
        let options = EncryptOptions {
            user_password: "buka-123".into(),
            owner_password: "pemilik-456".into(),
            permissions: EncryptPermissions {
                allow_print: true,
                allow_modify: false,
                allow_copy: false,
            },
        };
        let enc_report = sec.encrypt_pdf(&src, &locked, &options, &control)?;
        assert!(enc_report.encrypted);
        assert!(enc_report.bytes > 0);
        assert!(sec.is_encrypted(&locked, &control)?);

        // No-clobber refuses existing destination.
        assert_eq!(
            sec.encrypt_pdf(&src, &locked, &options, &control),
            Err(SecurityError::DestinationExists)
        );
        // Re-encrypting an already encrypted PDF is refused.
        let relocked = dir.join("relocked.pdf");
        assert_eq!(
            sec.encrypt_pdf(&locked, &relocked, &options, &control),
            Err(SecurityError::AlreadyEncrypted)
        );
        // Wrong password fails cleanly without creating destination.
        assert_eq!(
            sec.decrypt_pdf(&locked, &unlocked, "salah-pw", &control),
            Err(SecurityError::WrongPassword)
        );
        assert!(!unlocked.exists());

        // Correct user password unlocks cleanly.
        let dec_report = sec.decrypt_pdf(&locked, &unlocked, "buka-123", &control)?;
        assert!(!dec_report.encrypted);
        assert!(dec_report.bytes > 0);
        assert!(!sec.is_encrypted(&unlocked, &control)?);

        // Owner password also unlocks cleanly.
        let unlocked_owner = dir.join("unlocked-owner.pdf");
        let owner_report = sec.decrypt_pdf(&locked, &unlocked_owner, "pemilik-456", &control)?;
        assert!(!owner_report.encrypted);
        assert!(!sec.is_encrypted(&unlocked_owner, &control)?);

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn password_validation_rejects_empty_oversized_and_control_chars() {
        assert_eq!(
            validate_password("", false),
            Err(SecurityError::EmptyPassword)
        );
        assert_eq!(validate_password("", true), Ok(()));
        assert_eq!(
            validate_password("bad\npass", false),
            Err(SecurityError::InvalidPassword)
        );
        assert_eq!(
            validate_password(&"a".repeat(MAX_PASSWORD_BYTES + 1), false),
            Err(SecurityError::InvalidPassword)
        );
        assert_eq!(validate_password("rahasia 2026!", false), Ok(()));
    }
}
