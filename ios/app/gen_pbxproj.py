import hashlib
import pathlib

def oid(name: str) -> str:
    return hashlib.sha1(name.encode()).hexdigest()[:24].upper()

ids = {k: oid(k) for k in [
    "project", "target", "mainGroup", "appGroup", "productsGroup", "mainM", "infoPlist",
    "productApp", "sourcesPhase", "frameworksPhase", "resourcesPhase", "scriptPhase",
    "projectConfigList", "targetConfigList", "projectDebug", "projectRelease", "targetDebug",
    "targetRelease", "buildFileMain",
]}

SCRIPT = r'''set -euo pipefail
if [ "${PLATFORM_NAME}" = "iphonesimulator" ]; then
  RUST_TARGET=aarch64-apple-ios-sim
else
  RUST_TARGET=aarch64-apple-ios
fi
CARGO="${JASPER_CARGO:-$(command -v cargo || true)}"
if [ -z "${CARGO}" ]; then CARGO="${HOME}/.cargo/bin/cargo"; fi
export CARGO_TARGET_DIR="${SRCROOT}/../target"
"${CARGO}" build --locked --release --target "${RUST_TARGET}" --manifest-path "${SRCROOT}/../Cargo.toml"
mkdir -p "${SRCROOT}/../target/xcode/lib"
cp "${CARGO_TARGET_DIR}/${RUST_TARGET}/release/libjasper_ui_ios.a" "${SRCROOT}/../target/xcode/lib/libjasper_ui_ios.a"
echo "rust staticlib: ${RUST_TARGET}"'''

script_escaped = SCRIPT.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")

SETTINGS_COMMON = """				ALWAYS_SEARCH_USER_PATHS = NO;
				CLANG_ENABLE_MODULES = YES;
				CLANG_ENABLE_OBJC_ARC = YES;
				COPY_PHASE_STRIP = NO;
				IPHONEOS_DEPLOYMENT_TARGET = 15.0;
				SDKROOT = iphoneos;
				ENABLE_USER_SCRIPT_SANDBOXING = NO;
				SWIFT_VERSION = 5.0;"""

TARGET_COMMON = """				CODE_SIGN_STYLE = Automatic;
				CURRENT_PROJECT_VERSION = 1;
				DEVELOPMENT_TEAM = "$(JASPER_TEAM)";
				GENERATE_INFOPLIST_FILE = NO;
				INFOPLIST_FILE = Info.plist;
				LD_RUNPATH_SEARCH_PATHS = ("$(inherited)", "@executable_path/Frameworks");
				LIBRARY_SEARCH_PATHS = ("$(SRCROOT)/../target/xcode/lib");
				MARKETING_VERSION = 0.1.0;
				OTHER_LDFLAGS = (
					"-ljasper_ui_ios",
					"-framework", UIKit,
					"-framework", Metal,
					"-framework", QuartzCore,
					"-framework", CoreGraphics,
					"-framework", CoreFoundation,
					"-framework", Foundation,
					"-liconv",
					"-lobjc",
				);
				PRODUCT_BUNDLE_IDENTIFIER = org.jasper.ui.ios;
				PRODUCT_NAME = JasperUI;
				TARGETED_DEVICE_FAMILY = "1,2";"""

pbx = f'''// !$*UTF8*$!
{{
	archiveVersion = 1;
	classes = {{
	}};
	objectVersion = 56;
	objects = {{

/* Begin PBXBuildFile section */
		{ids['buildFileMain']} /* main.m in Sources */ = {{isa = PBXBuildFile; fileRef = {ids['mainM']} /* main.m */; }};
/* End PBXBuildFile section */

/* Begin PBXFileReference section */
		{ids['mainM']} /* main.m */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode.c.objc; path = main.m; sourceTree = "<group>"; }};
		{ids['infoPlist']} /* Info.plist */ = {{isa = PBXFileReference; lastKnownFileType = text.plist.xml; path = Info.plist; sourceTree = "<group>"; }};
		{ids['productApp']} /* JasperUI.app */ = {{isa = PBXFileReference; explicitFileType = wrapper.application; includeInIndex = 0; path = JasperUI.app; sourceTree = BUILT_PRODUCTS_DIR; }};
/* End PBXFileReference section */

/* Begin PBXFrameworksBuildPhase section */
		{ids['frameworksPhase']} /* Frameworks */ = {{
			isa = PBXFrameworksBuildPhase;
			buildActionMask = 2147483647;
			files = (
			);
			runOnlyForDeploymentPostprocessing = 0;
		}};
/* End PBXFrameworksBuildPhase section */

/* Begin PBXGroup section */
		{ids['mainGroup']} = {{
			isa = PBXGroup;
			children = (
				{ids['appGroup']} /* app */,
				{ids['productsGroup']} /* Products */,
			);
			sourceTree = "<group>";
		}};
		{ids['appGroup']} /* app */ = {{
			isa = PBXGroup;
			children = (
				{ids['mainM']} /* main.m */,
				{ids['infoPlist']} /* Info.plist */,
			);
			path = .;
			sourceTree = "<group>";
		}};
		{ids['productsGroup']} /* Products */ = {{
			isa = PBXGroup;
			children = (
				{ids['productApp']} /* JasperUI.app */,
			);
			name = Products;
			sourceTree = "<group>";
		}};
/* End PBXGroup section */

/* Begin PBXNativeTarget section */
		{ids['target']} /* JasperUI */ = {{
			isa = PBXNativeTarget;
			buildConfigurationList = {ids['targetConfigList']} /* Build configuration list for PBXNativeTarget "JasperUI" */;
			buildPhases = (
				{ids['scriptPhase']} /* Build Rust static library */,
				{ids['sourcesPhase']} /* Sources */,
				{ids['frameworksPhase']} /* Frameworks */,
				{ids['resourcesPhase']} /* Resources */,
			);
			buildRules = (
			);
			dependencies = (
			);
			name = JasperUI;
			productName = JasperUI;
			productReference = {ids['productApp']} /* JasperUI.app */;
			productType = "com.apple.product-type.application";
		}};
/* End PBXNativeTarget section */

/* Begin PBXProject section */
		{ids['project']} /* Project object */ = {{
			isa = PBXProject;
			attributes = {{
				BuildIndependentTargetsInParallel = 1;
				LastUpgradeCheck = 2600;
				TargetAttributes = {{
					{ids['target']} = {{
						CreatedOnToolsVersion = 26.0;
					}};
				}};
			}};
			buildConfigurationList = {ids['projectConfigList']} /* Build configuration list for PBXProject "JasperUI" */;
			compatibilityVersion = "Xcode 14.0";
			developmentRegion = en;
			hasScannedForEncodings = 0;
			knownRegions = (
				en,
				Base,
			);
			mainGroup = {ids['mainGroup']};
			productRefGroup = {ids['productsGroup']} /* Products */;
			projectDirPath = "";
			projectRoot = "";
			targets = (
				{ids['target']} /* JasperUI */,
			);
		}};
/* End PBXProject section */

/* Begin PBXResourcesBuildPhase section */
		{ids['resourcesPhase']} /* Resources */ = {{
			isa = PBXResourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
			);
			runOnlyForDeploymentPostprocessing = 0;
		}};
/* End PBXResourcesBuildPhase section */

/* Begin PBXShellScriptBuildPhase section */
		{ids['scriptPhase']} /* Build Rust static library */ = {{
			isa = PBXShellScriptBuildPhase;
			alwaysOutOfDate = 1;
			buildActionMask = 2147483647;
			files = (
			);
			inputFileListPaths = (
			);
			inputPaths = (
			);
			name = "Build Rust static library";
			outputFileListPaths = (
			);
			outputPaths = (
			);
			runOnlyForDeploymentPostprocessing = 0;
			shellPath = /bin/sh;
			shellScript = "{script_escaped}";
		}};
/* End PBXShellScriptBuildPhase section */

/* Begin PBXSourcesBuildPhase section */
		{ids['sourcesPhase']} /* Sources */ = {{
			isa = PBXSourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
				{ids['buildFileMain']} /* main.m in Sources */,
			);
			runOnlyForDeploymentPostprocessing = 0;
		}};
/* End PBXSourcesBuildPhase section */

/* Begin XCBuildConfiguration section */
		{ids['projectDebug']} /* Debug */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
{SETTINGS_COMMON}
				ONLY_ACTIVE_ARCH = YES;
				DEBUG_INFORMATION_FORMAT = dwarf;
				GCC_OPTIMIZATION_LEVEL = 0;
				SWIFT_OPTIMIZATION_LEVEL = "-Onone";
			}};
			name = Debug;
		}};
		{ids['projectRelease']} /* Release */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
{SETTINGS_COMMON}
				DEBUG_INFORMATION_FORMAT = "dwarf-with-dsym";
				ENABLE_NS_ASSERTIONS = NO;
				SWIFT_COMPILATION_MODE = wholemodule;
			}};
			name = Release;
		}};
		{ids['targetDebug']} /* Debug */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
{TARGET_COMMON}
			}};
			name = Debug;
		}};
		{ids['targetRelease']} /* Release */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
{TARGET_COMMON}
			}};
			name = Release;
		}};
/* End XCBuildConfiguration section */

/* Begin XCConfigurationList section */
		{ids['projectConfigList']} /* Build configuration list for PBXProject "JasperUI" */ = {{
			isa = XCConfigurationList;
			buildConfigurations = (
				{ids['projectDebug']} /* Debug */,
				{ids['projectRelease']} /* Release */,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		}};
		{ids['targetConfigList']} /* Build configuration list for PBXNativeTarget "JasperUI" */ = {{
			isa = XCConfigurationList;
			buildConfigurations = (
				{ids['targetDebug']} /* Debug */,
				{ids['targetRelease']} /* Release */,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		}};
/* End XCConfigurationList section */
	}};
	rootObject = {ids['project']} /* Project object */;
}}
'''

path = pathlib.Path("ios/app/JasperUI.xcodeproj/project.pbxproj")
path.write_text(pbx)
print("wrote", path, len(pbx))
