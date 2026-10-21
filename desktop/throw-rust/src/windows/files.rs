//! Windows Search's disk-backed SystemIndex through native ADO COM.
//! No PowerShell, managed runtime, custom crawler or file-catalog copy.
use super::platform::ShellApartment;
use throw_launcher::files::{self, FileEntry, FILE_CANDIDATES};
use windows::{
    core::{IUnknown, Interface, BSTR, GUID, PCWSTR, VARIANT},
    Win32::System::Com::*,
};

struct Dispatch(IDispatch);

impl Dispatch {
    unsafe fn invoke(
        &self,
        name: &str,
        flags: DISPATCH_FLAGS,
        mut args: Vec<VARIANT>,
    ) -> windows::core::Result<VARIANT> {
        let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut id = 0;
        self.0
            .GetIDsOfNames(&GUID::zeroed(), &PCWSTR(name.as_ptr()), 1, 0, &mut id)?;
        // Automation arguments are passed in reverse order.
        args.reverse();
        let mut put_id = -3; // DISPID_PROPERTYPUT
        let params = DISPPARAMS {
            rgvarg: args.as_mut_ptr(),
            rgdispidNamedArgs: if flags == DISPATCH_PROPERTYPUT {
                &mut put_id
            } else {
                std::ptr::null_mut()
            },
            cArgs: args.len() as u32,
            cNamedArgs: u32::from(flags == DISPATCH_PROPERTYPUT),
        };
        let mut result = VARIANT::default();
        self.0.Invoke(
            id,
            &GUID::zeroed(),
            0,
            flags,
            &params,
            Some(&mut result),
            None,
            None,
        )?;
        Ok(result)
    }

    unsafe fn get(&self, name: &str) -> windows::core::Result<VARIANT> {
        self.invoke(name, DISPATCH_PROPERTYGET, vec![])
    }

    fn from_variant(value: &VARIANT) -> windows::core::Result<Self> {
        Ok(Self(IUnknown::try_from(value)?.cast()?))
    }
}

pub fn search(terms: &str) -> Result<Vec<FileEntry>, String> {
    let sql = files::sql(terms)?;
    unsafe { search_inner(&sql) }.map_err(|error| {
        format!("File search unavailable ({error}). Enable Windows Search and check Windows Settings → Searching Windows / Indexing Options. Apps still work.")
    })
}

unsafe fn search_inner(sql: &str) -> Result<Vec<FileEntry>, String> {
    // COM objects must be destroyed before uninitializing this worker apartment.
    let _apartment = ShellApartment::initialize()?;
    let run = || -> windows::core::Result<Vec<FileEntry>> {
        let clsid = CLSIDFromProgID(windows::core::w!("ADODB.Connection"))?;
        let connection = Dispatch(CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER)?);
        connection.invoke(
            "ConnectionTimeout",
            DISPATCH_PROPERTYPUT,
            vec![VARIANT::from(2i32)],
        )?;
        connection.invoke(
            "CommandTimeout",
            DISPATCH_PROPERTYPUT,
            vec![VARIANT::from(2i32)],
        )?;
        connection.invoke(
            "Open",
            DISPATCH_METHOD,
            vec![VARIANT::from(
                "Provider=Search.CollatorDSO;Extended Properties='Application=Windows';",
            )],
        )?;
        let recordset = Dispatch::from_variant(&connection.invoke(
            "Execute",
            DISPATCH_METHOD,
            vec![VARIANT::from(sql)],
        )?)?;
        let fields = Dispatch::from_variant(&recordset.get("Fields")?)?;
        let field = Dispatch::from_variant(&fields.invoke(
            "Item",
            DISPATCH_PROPERTYGET,
            vec![VARIANT::from(0i32)],
        )?)?;
        let mut entries = Vec::with_capacity(FILE_CANDIDATES);
        for _ in 0..FILE_CANDIDATES {
            if bool::try_from(&recordset.get("EOF")?)? {
                break;
            }
            let path = BSTR::try_from(&field.get("Value")?)?.to_string();
            let entry = FileEntry { path: path.into() };
            if entry.valid() && !entries.iter().any(|e: &FileEntry| e.path == entry.path) {
                entries.push(entry);
            }
            recordset.invoke("MoveNext", DISPATCH_METHOD, vec![])?;
        }
        recordset.invoke("Close", DISPATCH_METHOD, vec![])?;
        connection.invoke("Close", DISPATCH_METHOD, vec![])?;
        Ok(entries)
    };
    run().map_err(|error| format!("{:#x}", error.code().0))
}
