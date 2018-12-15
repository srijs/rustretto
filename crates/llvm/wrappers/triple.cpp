#include <llvm/ADT/StringRef.h>
#include <llvm/ADT/Triple.h>

extern "C" {
    void LLVMTripleGetMacOSXVersion(const char *triple, unsigned *major, unsigned *minor, unsigned *micro) {
        llvm::Triple(llvm::StringRef(triple)).getMacOSXVersion(*major, *minor, *micro);
    }
}
