fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/slotshift.ico");
        resource.set("ProductName", "Slotshift");
        resource.set("FileDescription", "Slotshift - Codex account launcher");
        resource.set(
            "LegalCopyright",
            "Copyright 2026 zombie.digital contributors",
        );
        resource
            .compile()
            .expect("Could not compile the Windows icon/version resource");
    }
}
