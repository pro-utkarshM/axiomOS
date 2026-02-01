#![no_std]

pub const STRUCTURE: Dir<'static> = Dir::new(
    "",
    &[
        Dir::new(
            "bin",
            &[],
            &[
                File::new("init", Kind::Executable),
                File::new("gpio_demo", Kind::Executable),
                File::new("iio_demo", Kind::Executable),
                File::new("syscall_demo", Kind::Executable),
                File::new("file_io_demo", Kind::Executable),
            ],
        ),
        Dir::new("dev", &[Dir::new("fd", &[], &[])], &[]),
        Dir::new("var", &[Dir::new("tmp", &[], &[])], &[]),
    ],
    &[],
);

pub struct Dir<'a> {
    pub name: &'a str,
    pub subdirs: &'a [Dir<'a>],
    pub files: &'a [File<'a>],
}

impl<'a> Dir<'a> {
    #[must_use]
    pub const fn new(name: &'a str, subdirs: &'a [Dir<'a>], files: &'a [File<'a>]) -> Self {
        Self {
            name,
            subdirs,
            files,
        }
    }
}

pub struct File<'a> {
    pub name: &'a str,
    pub kind: Kind,
}

impl<'a> File<'a> {
    #[must_use]
    pub const fn new(name: &'a str, kind: Kind) -> Self {
        Self { name, kind }
    }
}

pub enum Kind {
    Executable,
    Resource,
}
