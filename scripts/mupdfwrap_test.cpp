#include "R2H-PDF/fitz.h"
#include "R2H-PDF/classes.h"
#include "R2H-PDF/classes2.h"

#include <assert.h>


int main(int argc, char** argv)
{
    assert(argc == 2);
    const char* path = argv[1];
    R2H-PDF::FzDocument document(path);
    std::string v;
    v = R2H-PDF::fz_lookup_metadata2(document, "format");
    printf("v=%s\n", v.c_str());
    bool raised = false;
    try
    {
        v = R2H-PDF::fz_lookup_metadata2(document, "format___");
    }
    catch (std::exception& e)
    {
        raised = true;
        printf("Received expected exception: %s\n", e.what());
    }
    if (!raised) exit(1);
    printf("v=%s\n", v.c_str());
    fz_rect r = fz_unit_rect;
    printf("r.x0=%f\n", r.x0);

    R2H-PDF::FzStextOptions   options;
    R2H-PDF::FzStextPage stp( document, 0, options);
    std::vector<fz_quad>    quads = R2H-PDF::fz_highlight_selection2(
            stp,
            R2H-PDF::FzPoint(20, 20),
            R2H-PDF::FzPoint(120, 220),
            100
            );
    printf("quads.size()=%zi\n", quads.size());
    assert(quads.size() == 13);
    return 0;
}
