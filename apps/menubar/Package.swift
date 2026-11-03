// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "HyaMenuBar",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(name: "Hya", path: "Sources/Hya"),
        .testTarget(name: "HyaTests", dependencies: ["Hya"], path: "Tests/HyaTests"),
    ]
)
