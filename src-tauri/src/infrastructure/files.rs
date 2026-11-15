use std::{fs::{self,OpenOptions},io::Write,path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH}};

pub fn save_new(folder: &Path, name: &str, text: &str) -> Result<PathBuf,String> {
    if name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) || name.chars().any(char::is_control) || name.ends_with(['.', ' ']) {
        return Err("Invalid file name".into());
    }
    let ext=Path::new(name).extension().and_then(|s|s.to_str()).unwrap_or("");
    if !["txt","json","csv"].contains(&ext) {return Err("Only JSON, TXT and CSV are supported".into());}
    // Each export gets its own directory. Existing user files are never overwritten.
    let stamp=SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
    let dir=folder.join(format!("{}-{}",stamp,std::process::id()));
    fs::create_dir(&dir).map_err(|e|e.to_string())?;
    let tmp=dir.join("pending.tmp");
    let result=(|| {
        let mut f=OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e|e.to_string())?;
        f.write_all(b"\xef\xbb\xbf").and_then(|_|f.write_all(text.as_bytes())).and_then(|_|f.sync_all()).map_err(|e|e.to_string())?;
        drop(f);let target=dir.join(name);fs::rename(&tmp,&target).map_err(|e|e.to_string())?;Ok(target)
    })();
    if result.is_err(){let _=fs::remove_file(&tmp);let _=fs::remove_dir(&dir);}
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn rejects_path_traversal(){assert!(save_new(Path::new("."),"../outside.json","{}").is_err());}
    #[test] fn csv_exports_stay_in_a_unique_folder(){
        let root=std::env::temp_dir().join(format!("statistics-csv-test-{}",SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();
        let a=save_new(&root,"stages.csv","name;done\r\nA;1").unwrap();
        let b=save_new(&root,"stages.csv","name;done\r\nA;0").unwrap();
        assert_ne!(a,b);assert_eq!(fs::read(a).unwrap(),b"\xef\xbb\xbfname;done\r\nA;1");
        fs::remove_dir_all(root).unwrap();
    }
    #[test] fn writes_bom_without_overwrite(){
        let root=std::env::temp_dir().join(format!("production-cycle-test-{}",SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();
        let a=save_new(&root,"Заказ 1.json","{}").unwrap();let b=save_new(&root,"Заказ 1.json","{\"v\":2}").unwrap();
        assert_ne!(a,b);assert_eq!(fs::read(a).unwrap(),b"\xef\xbb\xbf{}");fs::remove_dir_all(root).unwrap();
    }
}
