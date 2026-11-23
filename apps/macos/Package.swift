// swift-tools-version:5.10
import PackageDescription

let package = Package(
    name: "RunIt",
    platforms: [.macOS(.v14)],
    targets: [
        .target(name: "CRunIt"),
        // Plain logic, testable without the Rust library.
        .target(name: "RunItText"),
        .executableTarget(name: "RunIt", dependencies: ["CRunIt", "RunItText"], linkerSettings: [
            .linkedFramework("Security"), .linkedFramework("SystemConfiguration"), .linkedFramework("IOKit"),
        ]),
        .testTarget(name: "RunItTextTests", dependencies: ["RunItText"]),
    ]
)
