use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::authority::{Book, Tree};
use crate::bonds::Bonds;
use crate::coinage::{Cool, Redemption};
use crate::digit::Digit;
use crate::error::IdeasError;
use crate::header::Header;
use crate::position::Position;

/// Standard directory and file names within an .idea package.
///
/// These constants define the on-disk layout of a `.idea` directory bundle.
pub mod names {
    /// The cleartext header file.
    pub const HEADER: &str = "Header.json";
    /// Directory containing digit JSON files.
    pub const CONTENT: &str = "Content";
    /// Directory containing ownership and provenance data.
    pub const AUTHORITY: &str = "Authority";
    /// Directory containing bond (reference) files.
    pub const BONDS: &str = "Bonds";
    /// Directory containing economic value data.
    pub const COINAGE: &str = "Coinage";
    /// Directory containing spatial position data.
    pub const POSITION: &str = "Position";
    /// Hidden directory for CRDT operation logs.
    pub const CRDT: &str = ".crdt";

    /// Ownership ledger within Authority/.
    pub const BOOK: &str = "book.json";
    /// Provenance tree within Authority/.
    pub const TREE: &str = "tree.json";
    /// Cool value data within Coinage/.
    pub const COOL: &str = "value.json";
    /// Redemption data within Coinage/.
    pub const REDEMPTION: &str = "redemption.json";
    /// Local bonds within Bonds/.
    pub const LOCAL_BONDS: &str = "local.json";
    /// Private relay bonds within Bonds/.
    pub const PRIVATE_BONDS: &str = "private.json";
    /// Public relay bonds within Bonds/.
    pub const PUBLIC_BONDS: &str = "public.json";
    /// Position data within Position/.
    pub const POSITION_FILE: &str = "position.json";
}

/// A fully loaded .idea package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaPackage {
    #[serde(skip)]
    pub path: PathBuf,
    pub header: Header,
    pub digits: HashMap<Uuid, Digit>,
    pub book: Option<Book>,
    pub tree: Option<Tree>,
    pub cool: Option<Cool>,
    pub redemption: Option<Redemption>,
    pub bonds: Option<Bonds>,
    pub position: Option<Position>,
}

impl IdeaPackage {
    /// Creates a new idea package in memory.
    pub fn new(path: PathBuf, header: Header, root_digit: Digit) -> Self {
        let mut digits = HashMap::new();
        digits.insert(root_digit.id(), root_digit);
        IdeaPackage {
            path,
            header,
            digits,
            book: None,
            tree: None,
            cool: None,
            redemption: None,
            bonds: None,
            position: None,
        }
    }

    /// Returns a copy with a book set.
    pub fn with_book(mut self, book: Book) -> Self {
        self.book = Some(book);
        self
    }

    /// Returns a copy with a tree set.
    pub fn with_tree(mut self, tree: Tree) -> Self {
        self.tree = Some(tree);
        self
    }

    /// Returns a copy with a cool value set.
    pub fn with_cool(mut self, cool: Cool) -> Self {
        self.cool = Some(cool);
        self
    }

    /// Returns a copy with a redemption set.
    pub fn with_redemption(mut self, redemption: Redemption) -> Self {
        self.redemption = Some(redemption);
        self
    }

    /// Returns a copy with bonds set.
    pub fn with_bonds(mut self, bonds: Bonds) -> Self {
        self.bonds = Some(bonds);
        self
    }

    /// Returns a copy with position set.
    pub fn with_position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }

    /// Returns a copy with an additional digit.
    pub fn with_digit(mut self, digit: Digit) -> Self {
        self.digits.insert(digit.id(), digit);
        self
    }

    /// The root digit, if it exists.
    pub fn root_digit(&self) -> Option<&Digit> {
        self.digits.get(&self.header.content.root_digit_id)
    }

    /// Write the package to disk as a .idea directory.
    pub fn save(&self) -> Result<(), IdeasError> {
        let base = &self.path;

        // Create the .idea directory
        std::fs::create_dir_all(base)?;

        // Write Header.json
        write_json(base, names::HEADER, &self.header)?;

        // Write Content/{uuid}.json for each digit
        let content_dir = base.join(names::CONTENT);
        std::fs::create_dir_all(&content_dir)?;
        for (id, digit) in &self.digits {
            let filename = format!("{id}.json");
            write_json(&content_dir, &filename, digit)?;
        }

        // Write Authority/ (optional)
        if self.book.is_some() || self.tree.is_some() {
            let auth_dir = base.join(names::AUTHORITY);
            std::fs::create_dir_all(&auth_dir)?;
            if let Some(book) = &self.book {
                write_json(&auth_dir, names::BOOK, book)?;
            }
            if let Some(tree) = &self.tree {
                write_json(&auth_dir, names::TREE, tree)?;
            }
        }

        // Write Bonds/ (optional)
        if let Some(bonds) = &self.bonds {
            let bonds_dir = base.join(names::BONDS);
            std::fs::create_dir_all(&bonds_dir)?;
            if let Some(local) = &bonds.local {
                write_json(&bonds_dir, names::LOCAL_BONDS, local)?;
            }
            if let Some(priv_bonds) = &bonds.private_bonds {
                write_json(&bonds_dir, names::PRIVATE_BONDS, priv_bonds)?;
            }
            if let Some(pub_bonds) = &bonds.public_bonds {
                write_json(&bonds_dir, names::PUBLIC_BONDS, pub_bonds)?;
            }
        }

        // Write Coinage/ (optional)
        if self.cool.is_some() || self.redemption.is_some() {
            let coin_dir = base.join(names::COINAGE);
            std::fs::create_dir_all(&coin_dir)?;
            if let Some(cool) = &self.cool {
                write_json(&coin_dir, names::COOL, cool)?;
            }
            if let Some(redemption) = &self.redemption {
                write_json(&coin_dir, names::REDEMPTION, redemption)?;
            }
        }

        // Write Position/ (optional)
        if let Some(position) = &self.position {
            let pos_dir = base.join(names::POSITION);
            std::fs::create_dir_all(&pos_dir)?;
            write_json(&pos_dir, names::POSITION_FILE, position)?;
        }

        Ok(())
    }

    /// Load a package from a .idea directory.
    pub fn load(path: &Path) -> Result<Self, IdeasError> {
        if !path.is_dir() {
            return Err(IdeasError::NotADirectory(path.display().to_string()));
        }

        // Read Header.json
        let header_path = path.join(names::HEADER);
        if !header_path.exists() {
            return Err(IdeasError::HeaderNotFound);
        }
        let header: Header = read_json(&header_path)?;

        // Read Content/{uuid}.json
        let content_dir = path.join(names::CONTENT);
        let mut digits = HashMap::new();
        if content_dir.is_dir() {
            for entry in std::fs::read_dir(&content_dir)? {
                let entry = entry?;
                let file_path = entry.path();
                if file_path.extension().is_some_and(|e| e == "json") {
                    let digit: Digit = read_json(&file_path)?;
                    digits.insert(digit.id(), digit);
                }
            }
        }

        // Read Authority/ (optional)
        let auth_dir = path.join(names::AUTHORITY);
        let book = read_optional_json(&auth_dir.join(names::BOOK))?;
        let tree = read_optional_json(&auth_dir.join(names::TREE))?;

        // Read Bonds/ (optional)
        let bonds_dir = path.join(names::BONDS);
        let local = read_optional_json(&bonds_dir.join(names::LOCAL_BONDS))?;
        let private_bonds = read_optional_json(&bonds_dir.join(names::PRIVATE_BONDS))?;
        let public_bonds = read_optional_json(&bonds_dir.join(names::PUBLIC_BONDS))?;
        let bonds = if local.is_some() || private_bonds.is_some() || public_bonds.is_some() {
            Some(Bonds {
                local,
                private_bonds,
                public_bonds,
            })
        } else {
            None
        };

        // Read Coinage/ (optional)
        let coin_dir = path.join(names::COINAGE);
        let cool = read_optional_json(&coin_dir.join(names::COOL))?;
        let redemption = read_optional_json(&coin_dir.join(names::REDEMPTION))?;

        // Read Position/ (optional)
        let pos_dir = path.join(names::POSITION);
        let position = read_optional_json(&pos_dir.join(names::POSITION_FILE))?;

        Ok(IdeaPackage {
            path: path.to_path_buf(),
            header,
            digits,
            book,
            tree,
            cool,
            redemption,
            bonds,
            position,
        })
    }

    /// Read only the header from a .idea directory (no decryption needed).
    pub fn read_header(path: &Path) -> Result<Header, IdeasError> {
        let header_path = path.join(names::HEADER);
        if !header_path.exists() {
            return Err(IdeasError::HeaderNotFound);
        }
        read_json(&header_path)
    }
}

fn write_json<T: serde::Serialize>(dir: &Path, filename: &str, data: &T) -> Result<(), IdeasError> {
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(dir.join(filename), json)?;
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, IdeasError> {
    let data = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn read_optional_json<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<Option<T>, IdeasError> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(read_json(path)?))
}
