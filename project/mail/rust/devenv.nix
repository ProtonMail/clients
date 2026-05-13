{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

{
  # On macOS, defer to the host Xcode toolchain (xcrun/xcodebuild/clang) for
  # iOS cross-compiles. devenv's defaults inject pkgs.apple-sdk into packages,
  # which sets DEVELOPER_DIR/SDKROOT and puts xcbuild's xcrun on PATH ahead of
  # the host one. Setting apple.sdk = null both removes that package and tells
  # the stdenv override to drop apple-sdk hooks from extraBuildInputs.
  apple.sdk = lib.mkIf pkgs.stdenv.isDarwin null;

  enterShell = ''
    echo "Helper scripts you can run to make your development richer:"
    echo
    ${pkgs.gnused}/bin/sed -e 's| |••|g' -e 's|=| |' <<EOF | ${pkgs.util-linuxMinimal}/bin/column -t | ${pkgs.gnused}/bin/sed -e 's|^|  |' -e 's|••| |g'
    ${lib.generators.toKeyValue { } (lib.mapAttrs (name: value: value.description) config.scripts)}
    EOF
  '';


  packages =
    with pkgs;
    [
      bashInteractive
      git-cliff
      php # For iCal
      php.unwrapped.dev # For iCal
      sql-formatter
      cargo-nextest
      cargo-insta
    ]
    ++ lib.optionals pkgs.stdenv.isDarwin (
      with pkgs;
      [
        xcodes # Selector of the xcode version
        findutils
        libiconv
      ]
    );

  languages = {
   rust = {
     enable = true;
     toolchainFile = ../../../rust-toolchain.toml;
   };


    go = {
      enable = true; # For PGP
    };

    python = {
      enable = true; # For changelog
      package = pkgs.python312;
      uv = {
        enable = true;
      };
    };

    php = {
      enable = true; # For iCal
    };
  };

  scripts = {
    proton-install-xcode = {
      description = "Installs Xcode.";
      binary = "bash";

      exec = ''
        xcodes install 26.2
      '';
    };

    proton-logs-ios = {
      description = "Shows the rust logs of the iOS app";
      binary = "bash";

      exec = ''
        pushd "$DEVENV_ROOT"

        xcrun simctl spawn "$DEVICE_ID" log stream \
              --predicate 'subsystem == "ch.protonmail.protonmail" AND category == "[Proton] Rust"' \
              --style syslog

        popd
      '';
    };

    proton-run-ios = {
      description = ''Builds the iOS project (but not the uniffi framework) and runs it on the simulator'';
      binary = "bash";

      exec = ''
        pushd "$DEVENV_ROOT"

        ./mail/mail-uniffi/ios/run-local.sh

        popd
      '';
    };
    proton-build-ios = {
      description = "Builds iOS uniffi framework and injects it to the iOS project";
      binary = "bash";

      exec = ''
        pushd "$DEVENV_ROOT"

        # Nix's cc-wrapper bakes -mmacos-version-min and a MacOSX sysroot
        # into clang invocations, which conflicts with the iOS target flags
        # that cc-rs build scripts and rustc's linker step emit. Point both
        # cc-rs (CC_*/CXX_*) and rustc's linker (CARGO_TARGET_*_LINKER) at
        # the host Xcode clang for iOS targets — the nix wrapper is bypassed
        # for these targets only; native builds still use the nix toolchain.
        ios_sim_clang="$(xcrun --sdk iphonesimulator -f clang)"
        ios_clang="$(xcrun --sdk iphoneos -f clang)"
        export CC_aarch64_apple_ios_sim="$ios_sim_clang"
        export CXX_aarch64_apple_ios_sim="$(xcrun --sdk iphonesimulator -f clang++)"
        export CC_aarch64_apple_ios="$ios_clang"
        export CXX_aarch64_apple_ios="$(xcrun --sdk iphoneos -f clang++)"
        export CARGO_TARGET_AARCH64_APPLE_IOS_SIM_LINKER="$ios_sim_clang"
        export CARGO_TARGET_AARCH64_APPLE_IOS_LINKER="$ios_clang"

        ./mail/mail-uniffi/ios/build-local.sh

        popd
      '';
    };
  };
}
