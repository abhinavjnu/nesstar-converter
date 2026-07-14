//! Isolated WP-S3 feasibility spike.  It deliberately has no production API.
use std::{collections::BTreeMap, env, fs, path::{Path, PathBuf}};

const MAGIC: &[u8; 8] = b"NESSTART";
const SLOT: usize = 160;
const DBL_MAX: f64 = 1.7976931348623157e308;

#[derive(Clone, Debug)]
struct Var { name: String, numeric: bool, width: usize, decimals: usize, min: i64 }
#[derive(Clone, Debug)]
struct Block { id: String, rows: usize, vars: Vec<Var> }
#[derive(Clone, Debug)]
enum Encoding { Fixed(usize), CString(usize), Offset(usize, i64), Double, Compact(u8, i64), RawByte }
#[derive(Clone, Debug)]
struct Column { name: String, start: usize, size: usize, encoding: Encoding }
#[derive(Clone, Debug)]
struct Layout { rows: usize, columns: Vec<Column>, method: &'static str }
type Row = BTreeMap<String, String>;

fn err<T>(message: impl Into<String>) -> Result<T, String> { Err(message.into()) }
fn range<'a>(data: &'a [u8], start: usize, len: usize, what: &str) -> Result<&'a [u8], String> {
    let end = start.checked_add(len).ok_or_else(|| format!("{what}: offset overflow"))?;
    data.get(start..end).ok_or_else(|| format!("{what}: range {start}..{end} exceeds {} bytes", data.len()))
}
fn u16le(data: &[u8], at: usize) -> Result<u16, String> { Ok(u16::from_le_bytes(range(data, at, 2, "u16")?.try_into().unwrap())) }
fn u32le(data: &[u8], at: usize) -> Result<u32, String> { Ok(u32::from_le_bytes(range(data, at, 4, "u32")?.try_into().unwrap())) }
fn u48le(data: &[u8], at: usize) -> Result<usize, String> { let b = range(data, at, 6, "u48")?; Ok((b[0] as usize) | ((b[1] as usize)<<8) | ((b[2] as usize)<<16) | ((b[3] as usize)<<24) | ((b[4] as usize)<<32) | ((b[5] as usize)<<40)) }
fn i64le(data: &[u8], at: usize) -> Result<i64, String> { Ok(i64::from_le_bytes(range(data, at, 8, "i64")?.try_into().unwrap())) }
fn attr(tag: &str, key: &str) -> Option<String> { let needle = format!("{key}=\""); let i = tag.find(&needle)? + needle.len(); Some(tag[i..].split('"').next()?.to_string()) }
fn tag_value(text: &str, tag: &str) -> Option<String> { let open = format!("<{tag}>"); let start = text.find(&open)? + open.len(); Some(text[start..].split(&format!("</{tag}>")).next()?.trim().to_string()) }
fn tags<'a>(text: &'a str, name: &str) -> Vec<&'a str> { let needle = format!("<{name}"); text.match_indices(&needle).filter_map(|(i,_)| text[i..].find('>').map(|e| &text[i..i+e+1])).collect() }
fn parse_ddi(path: &Path) -> Result<Vec<Block>, String> {
    let xml = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut blocks = Vec::new();
    for fd in tags(&xml, "fileDscr") {
        let id = attr(fd, "ID").unwrap_or_default();
        let remainder = &xml[xml.find(fd).unwrap()..];
        let close = remainder.find("</fileDscr>").unwrap_or(fd.len());
        let scope = &remainder[..close];
        blocks.push(Block { id, rows: tag_value(scope, "caseQnty").and_then(|x|x.parse().ok()).unwrap_or(0), vars: Vec::new() });
    }
    for start in xml.match_indices("<var ").map(|(i,_)|i).collect::<Vec<_>>() {
        let end = xml[start..].find("</var>").ok_or("unterminated var")? + start + 6;
        let scope = &xml[start..end]; let head = &scope[..scope.find('>').ok_or("invalid var")?+1];
        let name = attr(head, "name").unwrap_or_default(); let files = attr(head, "files").unwrap_or_default();
        let location = tags(scope, "location").first().copied().unwrap_or("");
        let format = tags(scope, "varFormat").first().copied().unwrap_or("");
        let range_tag = tags(scope, "range").first().copied().unwrap_or("");
        let var = Var { name, numeric: attr(format,"type").as_deref()==Some("numeric"), width: attr(location,"width").and_then(|x|x.parse().ok()).unwrap_or(0), decimals: attr(format,"dcml").and_then(|x|x.parse().ok()).unwrap_or(0), min: attr(range_tag,"min").and_then(|x|x.parse::<f64>().ok()).unwrap_or(0.0) as i64 };
        for id in files.split_whitespace() { if let Some(block)=blocks.iter_mut().find(|b|b.id==id) { block.vars.push(var.clone()); } }
    }
    if blocks.is_empty() { err("DDI has no fileDscr blocks") } else { Ok(blocks) }
}
fn utf16_name(slot: &[u8], at: usize, length: usize) -> String { let raw=&slot[at..at+length]; String::from_utf16_lossy(&raw.chunks_exact(2).map(|p|u16::from_le_bytes([p[0],p[1]])).take_while(|x|*x!=0).collect::<Vec<_>>()) }
fn metadata_layout(data: &[u8], block: &Block) -> Result<Layout, String> {
    let first=block.vars.first().ok_or("metadata block has no variables")?;
    let needle: Vec<u8>=first.name.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let pos=data.windows(needle.len()).position(|w|w==needle).ok_or("metadata variable name not found")?;
    let meta=pos.checked_sub(63).ok_or("metadata name occurs before slot")?;
    let mut parsed=Vec::new();
    for i in 0..block.vars.len() { let s=range(data,meta+i*SLOT,SLOT,"metadata slot")?; parsed.push((u32le(s,0)?, s[4],s[5],s[14] as usize,utf16_name(s,63,80))); }
    let mut binary=Vec::new();
    for (v,(_,b4,b5,width,name)) in block.vars.iter().zip(parsed.iter()) {
        if !name.starts_with(&v.name) && !v.name.starts_with(name) { return err(format!("metadata slot name {name:?} does not match DDI {}",v.name)); }
        let enc=if *b4==1 { Encoding::Fixed(*width) } else if *b5==10 { Encoding::Double } else { let max_width=if v.width==0 {1} else {((10usize.pow(v.width as u32)-1).ilog2() as usize+8)/8}; Encoding::Offset(max_width,v.min) }; binary.push((v.name.clone(),enc));
    }
    binary.sort_by_key(|(name,_)| parsed.iter().position(|x|&x.4==name).unwrap_or(usize::MAX));
    let total:usize=binary.iter().map(|(_,e)|match e {Encoding::Fixed(w)|Encoding::Offset(w,_)=>*w,Encoding::Double=>8,_=>0}).sum();
    let bytes=total.checked_mul(block.rows).ok_or("metadata data size overflow")?; let data_start=meta.checked_sub(bytes).ok_or("metadata data starts before file")?;
    let mut cursor=data_start; let mut columns=Vec::new(); for (name,enc) in binary { let width=match enc {Encoding::Fixed(w)|Encoding::Offset(w,_)=>w,Encoding::Double=>8,_=>0}; let size=width*block.rows; range(data,cursor,size,"metadata payload")?; columns.push(Column{name,start:cursor,size,encoding:enc}); cursor+=size; }
    columns.sort_by_key(|c|block.vars.iter().position(|v|v.name==c.name).unwrap()); Ok(Layout{rows:block.rows,columns,method:"metadata_scan"})
}
fn resource_layout(data: &[u8], block: &Block) -> Result<Layout, String> {
    let index_at=u48le(data,0x25)?; let count=u32le(data,index_at)? as usize; let mut records=BTreeMap::new();
    for i in 0..count { let at=index_at+4+i*15; records.insert(u32le(data,at)?,(u48le(data,at+4)?,u32le(data,at+10)? as usize)); }
    let descriptor_id=u32le(data,0x2f)?; let (descriptor,_)=*records.get(&descriptor_id).ok_or("descriptor record missing")?; let datasets=data.get(0x2b).copied().unwrap_or(0) as usize; let descriptor_size=u16le(data,0x2d)? as usize;
    let mut chosen=None; for i in 0..datasets { let at=descriptor+i*descriptor_size; let vars=u32le(data,at+4)? as usize; let rows=u32le(data,at+8)? as usize; let entry_size=u16le(data,at+20)? as usize; let directory_id=u32le(data,at+22)?; if rows==block.rows && vars==block.vars.len() { chosen=Some((rows,vars,entry_size,*records.get(&directory_id).ok_or("directory record missing")?)); break; } }
    let (rows,vars,entry_size,(directory,_))=chosen.ok_or("no resource descriptor matches DDI")?; if entry_size<SLOT{return err("resource entry size below 160")}; let mut entries=BTreeMap::new();
    for i in 0..vars { let e=range(data,directory+i*entry_size,entry_size,"resource directory")?; let name=utf16_name(e,63,64); let id=u32le(e,15)?; let (start,size)=*records.get(&id).ok_or_else(||format!("payload record {id} missing"))?; entries.insert(name,(start,size,e[149] as usize,e[159],e[5],i64le(e,6)?)); }
    let mut columns=Vec::new(); for v in &block.vars { let (start,size,width,mode,format,offset)=entries.get(&v.name).copied().ok_or_else(||format!("resource directory lacks {}",v.name))?; range(data,start,size,"resource payload")?; let compact=match format {2=>Some((rows+1)/2),3=>Some(rows),4=>Some(rows*2),5=>Some(rows*3),6=>Some(rows*4),7=>Some(rows*5),10=>Some(rows*8),_=>None}; let encoding=if mode==5 || (v.numeric&&compact==Some(size)) {Encoding::Compact(format,if mode==5{offset}else{0})} else if v.numeric&&width==1&&size==rows&&data[start..start+size].iter().filter(|b|**b<32 && **b!=0 && **b!=9 && **b!=10 && **b!=13).count()*4>rows {Encoding::RawByte} else if mode==1 {Encoding::CString(width)} else {Encoding::Fixed(width)}; columns.push(Column{name:v.name.clone(),start,size,encoding}); }
    Ok(Layout{rows,columns,method:"resource_index"})
}
fn float_text(value:f64, decimals:usize)->String { if value.is_nan() || value>=DBL_MAX*0.99 {String::new()} else if value.fract()==0.0 {format!("{}",value as i64)} else {let s=format!("{value:.precision$}",precision=decimals.max(6));s.trim_end_matches('0').trim_end_matches('.').to_string()} }
fn decode_value(data:&[u8], column:&Column,row:usize,decimals:usize)->Result<String,String>{match &column.encoding {Encoding::Fixed(w)=>{let r=range(data,column.start+row**w,*w,"fixed value")?;Ok(String::from_utf8_lossy(r).replace('\0'," ").trim().to_string())},Encoding::CString(w)=>{let r=range(data,column.start+row**w,*w,"cstring value")?;Ok(String::from_utf8_lossy(&r[..r.iter().position(|x|*x==0).unwrap_or(*w)]).trim().to_string())},Encoding::Offset(w,offset)=>{let r=range(data,column.start+row**w,*w,"offset value")?;if r.iter().all(|x|*x==255){Ok(String::new())}else{let mut n=0u64;for(i,b)in r.iter().enumerate(){n|=(*b as u64)<<(i*8)};Ok((n as i64+*offset).to_string())}},Encoding::Double=>{let r=range(data,column.start+row*8,8,"double value")?;Ok(float_text(f64::from_le_bytes(r.try_into().unwrap()),decimals))},Encoding::RawByte=>{let b=range(data,column.start+row,1,"raw-byte value")?[0];Ok(if b==255{String::new()}else{b.to_string()})},Encoding::Compact(code,add)=>{let (n,missing)=match *code {2=>{let b=range(data,column.start+row/2,1,"nibble value")?[0];((if row%2==0{b>>4}else{b&15}) as u64,15u64)},3=>(range(data,column.start+row,1,"u8 value")?[0] as u64,255),4|5|6|7=>{let w=(*code-2)as usize;let r=range(data,column.start+row*w,w,"compact integer")?;let mut x=0;for(i,b)in r.iter().enumerate(){x|=(*b as u64)<<(i*8)};(x,(1u64<<(w*8))-1)},10=>{let r=range(data,column.start+row*8,8,"compact double")?;return Ok(float_text(f64::from_le_bytes(r.try_into().unwrap()),decimals));},_=>return err(format!("unsupported compact format {code}"))};Ok(if n==missing{String::new()}else{(n as i64+*add).to_string()})}}}
fn batches(data:&[u8], block:&Block, layout:&Layout, batch_rows:usize, mut keep:impl FnMut(usize)->bool)->Result<(Vec<Row>,usize),String>{let mut rows=Vec::new();let mut peak=0;for start in (0..layout.rows).step_by(batch_rows.max(1)){if !keep(start/batch_rows.max(1)){break}let end=(start+batch_rows.max(1)).min(layout.rows);let mut batch=Vec::with_capacity(end-start);for row in start..end{let mut out=Row::new();for column in &layout.columns{let decimals=block.vars.iter().find(|v|v.name==column.name).map(|v|v.decimals).unwrap_or(0);out.insert(column.name.clone(),decode_value(data,column,row,decimals)?);}batch.push(out);}let owned=batch.iter().flat_map(|r|r.iter()).map(|(k,v)|k.len()+v.len()).sum();peak=peak.max(owned);rows.extend(batch);}Ok((rows,peak))}
fn json_rows(path:&Path)->Result<Vec<Row>,String>{let text=fs::read_to_string(path).map_err(|e|e.to_string())?;let mut rows=Vec::new();for object in text.split('{').skip(1){let body=object.split('}').next().unwrap_or("");let mut row=Row::new();let q:Vec<&str>=body.split('"').skip(1).step_by(2).collect();for pair in q.chunks(2){if pair.len()==2{row.insert(pair[0].to_string(),pair[1].to_string());}}if !row.is_empty(){rows.push(row)}}Ok(rows)}
fn root()->PathBuf { PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..") }
fn run_one(stem:&str,batch:usize)->Result<usize,String>{let root=root();let ddi=root.join(format!("fixtures/synthetic/{stem}.ddi.xml"));let bin=root.join(format!("fixtures/synthetic/{stem}.Nesstar"));let expected=root.join(format!("fixtures/expected/{stem}.json"));let blocks=parse_ddi(&ddi)?;let block=blocks.first().ok_or("no DDI block")?;let data=fs::read(&bin).map_err(|e|e.to_string())?;if data.get(..8)!=Some(MAGIC){return err("invalid magic")};let layout=if stem=="metadata-scan"{metadata_layout(&data,block)?}else{resource_layout(&data,block)?};let (actual,peak)=batches(&data,block,&layout,batch, |_|true)?;let want=json_rows(&expected)?;if actual!=want{return err(format!("{stem}: decoded cells differ from expected JSON"))};println!("{stem}: layout={}, rows={}, batch_rows={batch}, peak_owned_batch_bytes={peak}",layout.method,actual.len());Ok(peak)}
fn main(){let batch=env::args().skip_while(|x|x!="--batch").nth(1).and_then(|x|x.parse().ok()).unwrap_or(2);for stem in ["metadata-scan","resource-index"]{if let Err(e)=run_one(stem,batch){eprintln!("error: {e}");std::process::exit(1)}}}

#[cfg(test)] mod tests { use super::*; #[test] fn both_layouts_match_json_at_multiple_batch_sizes(){for n in [1,2,64]{for stem in ["metadata-scan","resource-index"]{run_one(stem,n).unwrap();}}} #[test] fn cancellation_happens_between_batches(){let root=root();let block=parse_ddi(&root.join("fixtures/synthetic/resource-index.ddi.xml")).unwrap().remove(0);let data=fs::read(root.join("fixtures/synthetic/resource-index.Nesstar")).unwrap();let layout=resource_layout(&data,&block).unwrap();let (rows,_)=batches(&data,&block,&layout,2,|batch|batch==0).unwrap();assert_eq!(rows.len(),2);} #[test] fn malformed_inputs_error(){let root=root();let bad=fs::read(root.join("fixtures/malformed/bad-magic.Nesstar")).unwrap();assert_ne!(bad.get(..8),Some(MAGIC.as_slice()));let truncated=fs::read(root.join("fixtures/malformed/truncated-resource.Nesstar")).unwrap();let block=parse_ddi(&root.join("fixtures/synthetic/resource-index.ddi.xml")).unwrap().remove(0);assert!(resource_layout(&truncated,&block).is_err());} }
