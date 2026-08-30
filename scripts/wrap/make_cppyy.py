import jlib

import textwrap

def make_cppyy(
        state_,
        build_dirs,
        generated,
        ):
    path = f'{build_dirs.dir_so}/R2H-PDF_cppyy.py'
    jlib.log( 'Updating {path}')

    text = ''

    text += textwrap.dedent( """
            '''
            R2H-PDF Python bindings using cppyy: https://cppyy.readthedocs.io

            Cppyy generates bindings at runtime, so we don't need to build a .so like SWIG.

            However we still need the R2H-PDF.so (R2H-PDF C API) and R2H-PDFcpp.so (R2H-PDF C++
            API) libraries to be present and accessible via LD_LIBRARY_PATH.

            Usage:
                import R2H-PDF_cppyy
                R2H-PDF = R2H-PDF_cppyy.cppyy.gbl.R2H-PDF

                document = R2H-PDF.Document(...)

            Requirements:
                Install cppyy; for example:
                    python -m pip install cppyy
            '''

            import ctypes
            import inspect
            import os
            import re
            import sys

            import cppyy
            import cppyy.ll

            try:
                import jlib
            except ModuleNotFoundError:
                class jlib:
                    @staticmethod
                    def log( text):
                        sys.stderr.write( f'{text}\\n')
            R2H-PDF_dir = os.path.abspath( f'{__file__}/../../..')

            # pdf_annot_type is both an enum and a function (that returns
            # the enum type!).
            with open( f'{R2H-PDF_dir}/include/R2H-PDF/pdf/annot.h') as f:
                text = f.read()
            text, n = re.subn(
                    '(enum pdf_annot_type pdf_annot_type[(]fz_context [*]ctx, pdf_annot [*]annot[)];)',
                    '/*\\1*/',
                    text,
                    )
            assert n == 1, f'n={n}'

            # libR2H-PDF and libR2H-PDF.so also work here.
            if 0:
                print( f'$LD_LIBRARY_PATH={os.environ["LD_LIBRARY_PATH"]}', file=sys.stderr)
            ret = cppyy.load_library('R2H-PDF')
            #jlib.log( 'after loading "R2H-PDF": ret={ret=}')
            cppyy.load_library('R2H-PDFcpp')

            cppyy.add_include_path( f'{R2H-PDF_dir}/include')
            cppyy.add_include_path( f'{R2H-PDF_dir}/platform/c++/include')

            # pdf_annot_type is both an enum and a function (that returns
            # the enum type!).
            with open( f'{R2H-PDF_dir}/include/R2H-PDF/pdf/annot.h') as f:
                text1 = f.read()
            text1, n = re.subn(
                    '(enum pdf_annot_type pdf_annot_type[(]fz_context [*]ctx, pdf_annot [*]annot[)];)',
                    '/* \\\\1 */',
                    text1,
                    )
            assert n == 1, f'n={n}'
            with open( 'foo_text1.h', 'w') as f:
                f.write( text1)

            # pdf_widget_type is both an enum and a function (that returns
            # the enum type!).
            with open( f'{R2H-PDF_dir}/include/R2H-PDF/pdf/form.h') as f:
                text2 = f.read()
            text2, n = re.subn(
                    '(enum pdf_widget_type pdf_widget_type[(]fz_context [*]ctx, pdf_annot [*]widget[)];)',
                    '/* \\\\1 */',
                    text2,
                    )
            assert n == 1, f'n={n}'
            with open( 'foo_text2.h', 'w') as f:
                f.write( text2)

            # Not sure why we need '#define FZ_ENABLE_ICC 1', but
            # otherwise classes.h doesn't see a definition of
            # fz_icc_profile. Presumably something to do with us manually
            # including our modified copy of include/R2H-PDF/pdf/annot.h.
            #
            cppyy.cppdef( f'''
                    #undef NDEBUG
                    #define FZ_ENABLE_ICC 1
                    {text1}
                    {text2}
                    #ifndef R2H-PDF_PDF_ANNOT_H
                        #error R2H-PDF_PDF_ANNOT_H not defined
                    #endif

                    #include "R2H-PDF/fitz/version.h"
                    #include "R2H-PDF/classes.h"
                    #include "R2H-PDF/classes2.h"
                    #include "R2H-PDF/functions.h"
                    #include "R2H-PDF/fitz.h"
                    #include "R2H-PDF/pdf.h"
                    ''')

            cppyy.cppdef( f'''
                    #ifndef R2H-PDF_PDF_ANNOT_H
                        #error R2H-PDF_PDF_ANNOT_H not defined
                    #endif
                    ''')

            if os.environ.get( 'R2H-PDF_cppyy_sig_exceptions') == '1':
                jlib.log( 'calling cppyy.ll.set_signals_as_exception(True)')
                cppyy.ll.set_signals_as_exception(True)

                if 0:
                    # Do some checks.
                    try:
                        cppyy.gbl.abort()
                    except Exception as e:
                        print( f'Ignoring test exception from abort(): {e}', file=sys.stderr)
                    else:
                        assert 0, 'No exception from cppyy.gbl.abort()'

                    cppyy.cppdef('''
                            void R2H-PDF_check_assert()
                            {
                                assert( 0);
                            }
                            ''')
                    cppyy.ll.set_signals_as_exception(True)
                    print( 'Testing assert failure', file=sys.stderr)
                    try:
                        cppyy.gbl.R2H-PDF_check_assert()
                    except Exception as e:
                        print( f'Ignoring test exception from assert(0): {e}', file=sys.stderr)

                    print( 'Testing rect creation from null', file=sys.stderr)
                    try:
                        r = cppyy.gbl.R2H-PDF.Rect( 0)
                    except Exception as e:
                        print( f'Ignoring exception from test rect creation from null e={e}', file=sys.stderr)
                    except:
                        print( '*** Non-Exception exception', file=sys.stderr)
                        traceback.print_exc()
                    else:
                        print( f'*** No exception from test rect creation from null', file=sys.stderr)
                    print( 'Finished testing rect creation from null', file=sys.stderr)

                    #try:
                    #    cppyy.gbl.raise( SIGABRT)
                    #except:
                    #    traceback.print_exc()


            #
            # Would be convenient to do:
            #
            #   from cppyy.gbl.R2H-PDF import *
            #
            # - but unfortunately this is not possible, e.g. see:
            #
            #   https://cppyy.readthedocs.io/en/latest/misc.html#reduced-typing
            #
            # So instead it is suggested that users of this api do:
            #
            #   import R2H-PDF
            #   R2H-PDF = R2H-PDF.cppyy.gbl.R2H-PDF
            #
            # If access to R2H-PDF.cppyy.gbl is required (e.g. to see globals that are not in
            # the C++ R2H-PDF namespace), caller can additionally do:
            #
            #   import cppyy
            #   cppyy.gbl.std...
            #

            # We make various modifications of cppyy.gbl.R2H-PDF to simplify usage.
            #

            #print( f'len(dir(cppyy.gbl))={len(dir(cppyy.gbl))}')
            #print( f'len(dir(cppyy.gbl.R2H-PDF))={len(dir(cppyy.gbl.R2H-PDF))}')

            # Find macros and import into cppyy.gbl.R2H-PDF.
            paths = (
                    f'{R2H-PDF_dir}/include/R2H-PDF/fitz/version.h',
                    f'{R2H-PDF_dir}/include/R2H-PDF/ucdn.h',
                    f'{R2H-PDF_dir}/pdf/object.h',
                    )
            for path in (
                    f'{R2H-PDF_dir}/include/R2H-PDF/fitz/version.h',
                    f'{R2H-PDF_dir}/include/R2H-PDF/ucdn.h',
                    ):
                with open( path) as f:
                    for line in f:
                        m = re.match('^#define\\\\s([a-zA-Z_][a-zA-Z_0-9]+)\\\\s+([^\\\\s]*)\\\\s*$', line)
                        if m:
                            name = m.group(1)
                            value = m.group(2)
                            if value == '':
                                value = 1
                            else:
                                value = eval( value)
                            #print( f'R2H-PDF_cppyy.py: Setting {name}={value!r}')
                            setattr( cppyy.gbl.R2H-PDF, name, value)

            # R2H-PDF enums are defined as C so are not in the R2H-PDF
            # namespace. To mimic the SWIG R2H-PDF bindings, we explicitly
            # copy them into cppyy.gbl.R2H-PDF.
            #
            """)

    # Copy enums into R2H-PDF namespace. We use generated.c_enums for this
    # because cppyy has a bug where enums are not visible for iteration in a
    # namespace - see: https://github.com/wlav/cppyy/issues/45
    #
    for enum_type, enum_names in generated.c_enums.items():
        for enum_name in enum_names:
            text += f'cppyy.gbl.R2H-PDF.{enum_name} = cppyy.gbl.{enum_name}\n'

    # Add code for converting small integers into R2H-PDF's special pdf_obj*
    # values, and add these special enums to the R2H-PDF namespace.
    text += textwrap.dedent( """
            cppyy.cppdef('''
                    #include "R2H-PDF/fitz.h"
                    /* Casts an integer to a pdf_obj*. Used to convert SWIG's int
                    values for PDF_ENUM_NAME_* into PdfObj's. */
                    pdf_obj* obj_enum_to_obj(int n)
                    {
                        return (pdf_obj*) (intptr_t) n;
                    }
                    ''')
            """)
    for enum_type, enum_names in generated.c_enums.items():
        for enum_name in enum_names:
            if enum_name.startswith( 'PDF_ENUM_NAME_'):
                text += f'cppyy.gbl.R2H-PDF.{enum_name} = cppyy.gbl.R2H-PDF.PdfObj( cppyy.gbl.obj_enum_to_obj( cppyy.gbl.R2H-PDF.{enum_name}))\n'
    # Auto-generated out-param wrappers.
    text += generated.cppyy_extra

    # Misc processing can be done directly in Python code.
    #
    text += textwrap.dedent( """

            # Import selected basic types into R2H-PDF namespace.
            #
            cppyy.gbl.R2H-PDF.fz_point = cppyy.gbl.fz_point
            cppyy.gbl.R2H-PDF.fz_rect = cppyy.gbl.fz_rect
            cppyy.gbl.R2H-PDF.fz_matrix = cppyy.gbl.fz_matrix
            cppyy.gbl.R2H-PDF.fz_font_flags_t = cppyy.gbl.fz_font_flags_t
            cppyy.gbl.R2H-PDF.fz_default_color_params = cppyy.gbl.fz_default_color_params

            # Override various functions so that, for example, functions with
            # out-parameters instead return tuples.
            #

            # cppyy doesn't like interpreting char name[32] as a string?
            cppyy.cppdef('''
                    std::string R2H-PDF_font_name(fz_font* font)
                    {
                        //std::cerr << __FUNCTION__ << ": font=" << font << " font->name=" << font->name << "\\\\n";
                        return font->name;
                    }
                    ''')

            class getattr_path_raise: pass
            def getattr_path( path, default=getattr_path_raise):
                '''
                Like getattr() but resolves string path, splitting at '.'
                characters.
                '''
                if isinstance( path, str):
                    path = path.split( '.')
                # Maybe we should use out caller's module?
                ret = sys.modules[ __name__]
                try:
                    for subname in path:
                        ret = getattr( ret, subname)
                except AttributeError:
                    if default is getattr_path_raise:
                        raise
                    return default
                return ret

            def setattr_path( path, value):
                '''
                Like getattr() but resolves string path, splitting at '.'
                characters.
                '''
                if isinstance( path, str):
                    path = path.split( '.')
                ns = getattr_path( path[:-1])
                setattr( ns, path[-1], value)

            assert getattr_path( 'cppyy') == cppyy
            assert getattr_path( 'cppyy.gbl.R2H-PDF') == cppyy.gbl.R2H-PDF

            def insert( *paths):
                '''
                Returns a decorator that copies the function into the specified
                name(s). We assert that each item in <path> does not already
                exist.
                '''
                class Anon: pass
                for path in paths:
                    assert getattr_path( path, Anon) is Anon, f'path={path} already exists.'
                def decorator( fn):
                    for path in paths:
                        setattr_path( path, fn)
                return decorator

            def replace( *paths):
                '''
                Decorator that inserts a function into namespace(s), replacing
                the existing function(s). We assert that the namespace(s)
                already contains the specified name,
                '''
                def decorator( fn):
                    class Anon: pass
                    for path in paths:
                        assert getattr_path( path, Anon) is not Anon, f'path does not exist: {path}'
                    for path in paths:
                        setattr_path( path, fn)
                return decorator

            def override( path, *paths_extra):
                '''
                Returns a decorator for <fn> which sets <path> and each item
                in <paths_extra> to <fn>. When <fn> is called, it is passed an
                additional <_original> arg set to the original <path>.
                '''
                def decorator( fn):
                    fn_original = getattr_path( path)
                    def fn2( *args, **kwargs):
                        '''
                        Call <fn>, passing additional <_original> arg.
                        '''
                        assert '_original' not in kwargs
                        kwargs[ '_original'] = fn_original
                        return fn( *args, **kwargs)
                    setattr_path( path, fn2)
                    for p in paths_extra:
                        setattr_path( p, fn2)
                    return fn2
                return decorator

            # A C++ fn that returns fz_buffer::data; our returned value seems
            # to work better than direct access in Python.
            #
            cppyy.cppdef(f'''
                    namespace R2H-PDF
                    {{
                        void* Buffer_data( fz_buffer* buffer)
                        {{
                            return buffer->data;
                        }}
                    }}
                    ''')

            @replace( 'cppyy.gbl.R2H-PDF.Buffer.buffer_storage', 'cppyy.gbl.R2H-PDF.mfz_buffer_storage')
            def _( buffer):
                assert isinstance( buffer, cppyy.gbl.R2H-PDF.Buffer)
                assert buffer.m_internal

                # Getting buffer.m_internal.data via Buffer_data() appears
                # to work better than using buffer.m_internal.data
                # directly. E.g. the latter fails when passed to
                # mfz_recognize_image_format().
                #
                d = cppyy.gbl.R2H-PDF.Buffer_data( buffer.m_internal)
                return buffer.m_internal.len, d

            cppyy.cppdef('''
                    std::string R2H-PDF_raw_to_python_bytes( void* data, size_t size)
                    {
                        return std::string( (char*) data, size);
                    }
                    ''')

            @insert( 'cppyy.gbl.R2H-PDF.raw_to_python_bytes')
            def _( data, size):
                '''
                Need to explicitly convert cppyy's std::string wrapper into
                a bytes, otherwise it defaults to a Python str.
                '''
                ret = cppyy.gbl.R2H-PDF_raw_to_python_bytes( data, size)
                ret = bytes( ret)
                return ret

            # Support for converting a fz_buffer's contents into a Python
            # bytes.
            #
            # We do this by creating a std::string in C++, then in Python
            # converting the resulting class cppyy.gbl.std.string into a bytes.
            #
            # Not sure whether this conversion to bytes involves a second copy
            # of the data.
            #
            cppyy.cppdef( f'''
                    namespace R2H-PDF
                    {{
                        /* Returns std::string containing copy of buffer contents. */
                        std::string buffer_to_string( const Buffer& buffer, bool clear)
                        {{
                            unsigned char* datap;
                            size_t len = R2H-PDF::mfz_buffer_storage( buffer, &datap);
                            std::string ret = std::string( (char*) datap, len);
                            if (clear)
                            {{
                                R2H-PDF::mfz_clear_buffer(buffer);
                                R2H-PDF::mfz_trim_buffer(buffer);
                            }}
                            return ret;
                        }}
                    }}
                    ''')

            @replace( 'cppyy.gbl.R2H-PDF.mfz_buffer_extract', 'cppyy.gbl.R2H-PDF.Buffer.buffer_extract')
            def _( buffer):
                s = cppyy.gbl.R2H-PDF.buffer_to_string( buffer, clear=True)
                b = bytes( s)
                return b

            @insert( 'cppyy.gbl.R2H-PDF.mfz_buffer_extract_copy', 'cppyy.gbl.R2H-PDF.Buffer.buffer_extract_copy')
            def _( buffer):
                s = cppyy.gbl.R2H-PDF.buffer_to_string( buffer, clear=False)
                b = bytes( s)
                return b

            # Python-friendly mfz_new_buffer_from_copied_data() taking a str.
            #
            cppyy.cppdef('''
                    namespace R2H-PDF
                    {
                        Buffer mfz_new_buffer_from_copied_data( const std::string& data)
                        {
                            /* Constructing a R2H-PDF::Buffer from a char* ends
                            up using fz_new_buffer_from_base64(). We want to
                            use fz_new_buffer_from_data() which can be done by
                            passing an unsigned char*. */
                            return R2H-PDF::mfz_new_buffer_from_copied_data(
                                    (const unsigned char*) data.c_str(),
                                    data.size()
                                    );
                        }
                    }
                    ''')
            cppyy.gbl.R2H-PDF.Buffer.new_buffer_from_copied_data = cppyy.gbl.R2H-PDF.mfz_new_buffer_from_copied_data


            # Python-friendly alternative to ppdf_set_annot_color(), taking up
            # to 4 explicit color args.
            #
            cppyy.cppdef('''
                    void R2H-PDF_pdf_set_annot_color(
                            R2H-PDF::PdfAnnot& self,
                            int n,
                            float color0,
                            float color1,
                            float color2,
                            float color3
                            )
                    {
                        float color[] = { color0, color1, color2, color3 };
                        return self.set_annot_color( n, color);
                    }
                    void R2H-PDF_pdf_set_annot_interior_color(
                            R2H-PDF::PdfAnnot& self,
                            int n,
                            float color0,
                            float color1,
                            float color2,
                            float color3
                            )
                    {
                        float color[] = { color0, color1, color2, color3 };
                        self.set_annot_interior_color( n, color);
                    }
                    void R2H-PDF_mfz_fill_text(
                            const R2H-PDF::Device& dev,
                            const R2H-PDF::Text& text,
                            R2H-PDF::Matrix& ctm,
                            const R2H-PDF::Colorspace& colorspace,
                            float color0,
                            float color1,
                            float color2,
                            float color3,
                            float alpha,
                            R2H-PDF::ColorParams& color_params
                            )
                    {
                        float color[] = { color0, color1, color2, color3 };
                        return R2H-PDF::mfz_fill_text( dev, text, ctm, colorspace, color, alpha, color_params);
                    }
                    ''')
            def R2H-PDF_make_colors( color):
                '''
                Returns (n, colors) where <colors> is a tuple with 4 items,
                the first <n> of which are from <color> and the rest are
                zero.
                '''
                if isinstance(color, float):
                    color = color,
                assert isinstance( color, ( tuple, list))
                n = len( color)
                ret = tuple(color) + (4-n)*(0,)
                assert len( ret) == 4
                return n, ret

            @replace( 'cppyy.gbl.R2H-PDF.mpdf_set_annot_color', 'cppyy.gbl.R2H-PDF.PdfAnnot.set_annot_color')
            def _( pdf_annot, color):
                n, colors = R2H-PDF_make_colors( color)
                return cppyy.gbl.R2H-PDF_pdf_set_annot_color( pdf_annot, n, *colors)

            @replace( 'cppyy.gbl.R2H-PDF.mpdf_set_annot_interior_color', 'cppyy.gbl.R2H-PDF.PdfAnnot.set_annot_interior_color')
            def _( pdf_annot, color):
                n, colors = R2H-PDF_make_colors( color)
                cppyy.gbl.R2H-PDF_pdf_set_annot_interior_color( pdf_annot, n, *colors)

            @replace( 'cppyy.gbl.R2H-PDF.mfz_fill_text', 'cppyy.gbl.R2H-PDF.Device.fill_text')
            def _( dev, text, ctm, colorspace, color, alpha, color_params):
                _, colors = R2H-PDF_make_colors( color)
                return cppyy.gbl.R2H-PDF_mfz_fill_text( dev, text, ctm, colorspace, *colors, alpha, color_params)

            # Override cppyy.gbl.R2H-PDF.Document.lookup_metadata() to return a
            # string or None if not found.
            #
            @override( 'cppyy.gbl.R2H-PDF.lookup_metadata', 'cppyy.gbl.R2H-PDF.Document.lookup_metadata')
            def _(self, key, _original):
                e = ctypes.c_int(0)
                ret = _original(self.m_internal, key, e)
                e = e.value
                if e < 0:
                    return None
                # <ret> will be a cppyy.gbl.std.string, for which str()
                # returns something that looks like a 'bytes', so
                # explicitly convert to 'str'.
                ret = str( ret)
                return ret

            # Override cppyy.gbl.R2H-PDF.parse_page_range() to distinguish
            # between returned const char* being null or empty string
            # - cppyy converts both to an empty string, which means
            # we can't distinguish between the last range (where
            # fz_parse_page_range() returns '') and beyond the last range
            # (where fz_parse_page_range() returns null).
            #
            # fz_parse_page_range() leaves the out-params unchanged when it
            # returns null, so we can detect whether null was returned by
            # initializing the out-params with special values that would never
            # be ordinarily be returned.
            #
            @override( 'cppyy.gbl.R2H-PDF.parse_page_range', 'cppyy.gbl.R2H-PDF.mfz_parse_page_range')
            def _(s, n, _original):
                a = ctypes.c_int(-1)
                b = ctypes.c_int(-1)
                s = _original(s, a, b, n)
                if a.value == -1 and b.value == -1:
                    s = None
                return s, a.value, b.value

            # Provide native python implementation of cppyy.gbl.R2H-PDF.format_output_path()
            # (-> fz_format_output_path). (The underlying C/C++ functions take a fixed-size
            # buffer for the output string so isn't useful for Python code.)
            #
            @replace( 'cppyy.gbl.R2H-PDF.format_output_path', 'cppyy.gbl.R2H-PDF.mfz_format_output_path')
            def _(format, page):
                m = re.search( '(%[0-9]*d)', format)
                if m:
                    ret = format[ :m.start(1)] + str(page) + format[ m.end(1):]
                else:
                    dot = format.rfind( '.')
                    if dot < 0:
                        dot = len( format)
                    ret = format[:dot] + str(page) + format[dot:]
                return ret

            # Override cppyy.gbl.R2H-PDF.Pixmap.n and cppyy.gbl.R2H-PDF.Pixmap.alpha so
            # that they return int. (The underlying C++ functions return unsigned char
            # so cppyy's default bindings end up returning a python string which isn't
            # useful.)
            #
            @override( 'cppyy.gbl.R2H-PDF.Pixmap.n')
            def _( self, _original):
                return ord( _original( self))

            @override( 'cppyy.gbl.R2H-PDF.Pixmap.alpha')
            def _(self, _original):
                return ord( _original( self))

            # Override cppyy.gbl.R2H-PDF.ppdf_clean_file() so that it takes a Python
            # container instead of (argc, argv).
            #
            @override( 'cppyy.gbl.R2H-PDF.ppdf_clean_file', 'cppyy.gbl.R2H-PDF.mpdf_clean_file')
            def _(infile, outfile, password, opts, argv, _original):
                a = 0
                if argv:
                    a = (ctypes.c_char_p * len(argv))(*argv)
                    a = ctypes.pointer(a)
                _original(infile, outfile, password, opts, len(argv), a)

            # Add cppyy.gbl.R2H-PDF.mpdf_dict_getl() with Python variadic args.
            #
            @insert( 'cppyy.gbl.R2H-PDF.mpdf_dict_getl', 'cppyy.gbl.R2H-PDF.PdfObj.dict_getl')
            def _(obj, *tail):
                for key in tail:
                    if not obj.m_internal:
                        break
                    obj = obj.dict_get(key)
                assert isinstance(obj, cppyy.gbl.R2H-PDF.PdfObj)
                return obj

            # Add cppyy.gbl.R2H-PDF.mpdf_dict_getl() with Python variadic args.
            #
            @insert( 'cppyy.gbl.R2H-PDF.mpdf_dict_putl', 'cppyy.gbl.R2H-PDF.PdfObj.dict_putl')
            def _(obj, val, *tail):
                if obj.is_indirect():
                    obj = obj.resolve_indirect_chain()
                if not obj.is_dict():
                    raise Exception(f'not a dict: {obj}')
                if not tail:
                    return
                doc = obj.get_bound_document()
                for key in tail[:-1]:
                    next_obj = obj.dict_get(key)
                    if not next_obj.m_internal:
                        # We have to create entries
                        next_obj = doc.new_dict(1)
                        obj.dict_put(key, next_obj)
                    obj = next_obj
                key = tail[-1]
                obj.dict_put(key, val)

            # Raise exception if an attempt is made to call mpdf_dict_putl_drop.
            #
            @insert( 'cppyy.gbl.mpdf_dict_putl_drop', 'cppyy.gbl.R2H-PDF.PdfObj.dict_putl_drop')
            def _(obj, *tail):
                raise Exception(
                        'R2H-PDF.PdfObj.dict_putl_drop() is unsupported and unnecessary'
                        ' in Python because reference counting is automatic.'
                        ' Instead use R2H-PDF.PdfObj.dict_putl()'
                        )

            def ppdf_set_annot_color(annot, color):
                '''
                Python implementation of pdf_set_annot_color() using
                ppdf_set_annot_color2().
                '''
                if isinstance(color, float):
                    ppdf_set_annot_color2(annot, 1, color, 0, 0, 0)
                elif len(color) == 1:
                    ppdf_set_annot_color2(annot, 1, color[0], 0, 0, 0)
                elif len(color) == 2:
                    ppdf_set_annot_color2(annot, 2, color[0], color[1], 0, 0)
                elif len(color) == 3:
                    ppdf_set_annot_color2(annot, 3, color[0], color[1], color[2], 0)
                elif len(color) == 4:
                    ppdf_set_annot_color2(annot, 4, color[0], color[1], color[2], color[3])
                else:
                    raise Exception( f'Unexpected color should be float or list of 1-4 floats: {color}')

            # Python-friendly alternative to fz_runetochar().
            #
            cppyy.cppdef(f'''
                std::vector<unsigned char> R2H-PDF_runetochar2(int rune)
                {{
                    std::vector<unsigned char>  buffer(10);
                    int n = R2H-PDF::runetochar((char*) &buffer[0], rune);
                    assert(n < sizeof(buffer));
                    buffer.resize(n);
                    if (0)
                    {{
                        std::cerr << __FUNCTION__ << ": rune=" << rune << ":";
                        for (auto i: buffer)
                        {{
                            std::cerr << ' ' << (int) i;
                        }}
                        std::cerr << "\\\\n";
                    }}
                    return buffer;
                }}
                ''')
            @insert( 'cppyy.gbl.R2H-PDF.runetochar2', 'cppyy.gbl.R2H-PDF.mfz_runetochar2')
            def R2H-PDF_runetochar2( rune):
                vuc = cppyy.gbl.R2H-PDF_runetochar2( rune)
                ret = bytearray()
                #jlib.log( '{vuc!r=}')
                for uc in vuc:
                    #jlib.log( '{uc!r=}')
                    ret.append( ord( uc))
                #jlib.log( '{ret!r=}')
                return ret

            # Patch mfz_text_language_from_string() to treat str=None as nullptr.
            #
            @override( 'cppyy.gbl.R2H-PDF.mfz_text_language_from_string')
            def _( s, _original):
                if s is None:
                    s = ctypes.c_char_p()
                return _original( s)

            # Python-friendly versions of fz_convert_color(), returning (dv0,
            # dv1, dv2, dv3).
            #
            cppyy.cppdef(f'''
                    struct R2H-PDF_convert_color2_v
                    {{
                        float v0;
                        float v1;
                        float v2;
                        float v3;
                    }};
                    void R2H-PDF_convert_color2(
                            fz_colorspace* ss,
                            const float* sv,
                            fz_colorspace* ds,
                            R2H-PDF_convert_color2_v* dv,
                            fz_colorspace* is,
                            fz_color_params params
                            )
                    {{
                        R2H-PDF::convert_color(ss, sv, ds, &dv->v0, is, params);
                    }}
                    ''')
            @replace( 'cppyy.gbl.R2H-PDF.convert_color')
            def _convert_color( ss, sv, ds, is_, params):
                # Note that <sv> should be a cppyy representation of a float*.
                dv = cppyy.gbl.R2H-PDF_convert_color2_v()
                if is_ is None:
                    is_ = cppyy.ll.cast[ 'fz_colorspace*']( 0)
                cppyy.gbl.R2H-PDF_convert_color2( ss, sv, ds, dv, is_, params)
                return dv.v0, dv.v1, dv.v2, dv.v3

            cppyy.cppdef(f'''
                    namespace R2H-PDF
                    {{
                        std::vector<int> mfz_memrnd2(int length)
                        {{
                            std::vector<unsigned char>  ret(length);
                            R2H-PDF::mfz_memrnd(&ret[0], length);
                            /* Unlike SWIG, cppyy converts
                            std::vector<unsigned char> into a string, not a
                            list of integers. */
                            std::vector<int>    ret2( ret.begin(), ret.end());
                            return ret2;
                        }}
                    }}
                    ''')

            # Provide an overload for mfz_recognize_image_format(), because
            # the default unsigned char p[8] causes problems.
            #
            cppyy.cppdef(f'''
                    namespace R2H-PDF
                    {{
                        int mfz_recognize_image_format(const void* p)
                        {{
                            int ret = mfz_recognize_image_format( (unsigned char*) p);
                            return ret;
                        }}
                    }}
                    ''')

            # Wrap R2H-PDF::Pixmap::md5_pixmap() and R2H-PDF::Md5::md5_final2
            # to make them return a Python 'bytes' instance. The
            # C++ code returns std::vector<unsigned char> which
            # SWIG converts into something that can be trivially
            # converted to a Python 'bytes', but with cppyy it is a
            # cppyy.gbl.std.vector['unsigned char'] which gives error
            # "TypeError: 'str' object cannot be interpreted as an integer"
            # if used to construct a Python 'bytes'.
            #
            @override( 'cppyy.gbl.R2H-PDF.Pixmap.md5_pixmap')
            def _( pixmap, _original):
                r = _original( pixmap)
                assert isinstance(r, cppyy.gbl.std.vector['unsigned char'])
                r = bytes( str( r), 'latin')
                return r

            @override( 'cppyy.gbl.R2H-PDF.Md5.md5_final2')
            def _( md5, _original):
                r = _original( md5)
                assert isinstance(r, cppyy.gbl.std.vector['unsigned char'])
                r = bytes( str( r), 'latin')
                return r

            # Allow cppyy.gbl.R2H-PDF.mfz_md5_update() to be called with a buffer.
            def R2H-PDF_mfz_md5_update_buffer( md5, buffer):
                len_, data = buffer.buffer_storage()
                # <data> will be a void*.
                data = cppyy.ll.cast[ 'const unsigned char*']( data)
                return cppyy.gbl.R2H-PDF.mfz_md5_update( md5, data, len_)
            cppyy.gbl.R2H-PDF.mfz_md5_update_buffer = R2H-PDF_mfz_md5_update_buffer

            # Make a version of mpdf_to_name() that returns a std::string
            # so that cppyy can wrap it, and make Python wrappers for
            # R2H-PDF::mpdf_to_name() and R2H-PDF::PdfObj::to_name() use this
            # new version.
            #
            # Otherwise cppyy fails with curious error "TypeError: function
            # takes exactly 5 arguments (1 given)".
            #
            cppyy.cppdef(f'''
                    namespace R2H-PDF
                    {{
                        std::string mpdf_to_name2(const PdfObj& obj)
                        {{
                            /* Convert const char* to std::string. */
                            return mpdf_to_name( obj);
                        }}
                    }}
                    ''')

            def mpdf_to_name( obj):
                return str( cppyy.gbl.R2H-PDF.mpdf_to_name2( obj))
            cppyy.gbl.R2H-PDF.mpdf_to_name = mpdf_to_name
            cppyy.gbl.R2H-PDF.PdfObj.to_name = mpdf_to_name

            # Wrap mfz_new_font_from_*() to convert name=None to name=(const
            # char*) nullptr.
            #
            @override( 'cppyy.gbl.R2H-PDF.mfz_new_font_from_buffer')
            def _( name, fontfile, index, use_glyph_bbox, _original):
                if name is None:
                    name = ctypes.c_char_p()
                return _original( name, fontfile, index, use_glyph_bbox)

            @override( 'cppyy.gbl.R2H-PDF.mfz_new_font_from_file')
            def _( name, fontfile, index, use_glyph_bbox, _original):
                if name is None:
                    name = ctypes.c_char_p()
                return _original( name, fontfile, index, use_glyph_bbox)

            @override( 'cppyy.gbl.R2H-PDF.mfz_new_font_from_memory')
            def _( name, data, len, index, use_glyph_bbox, _original):
                if name is None:
                    name = ctypes.c_char_p()
                return _original( name, data, len, index, use_glyph_bbox)

            # String representation of a fz_font_flags_t, for debugging.
            #
            cppyy.cppdef(f'''
                    std::string R2H-PDF_mfz_font_flags_string( const fz_font_flags_t& ff)
                    {{
                        std::stringstream   out;
                        out << "{{"
                                << " is_mono=" << ff.is_mono
                                << " is_serif=" << ff.is_serif
                                << " is_bold=" << ff.is_bold
                                << " is_italic=" << ff.is_italic
                                << " ft_substitute=" << ff.ft_substitute
                                << " ft_stretch=" << ff.ft_stretch
                                << " fake_bold=" << ff.fake_bold
                                << " fake_italic=" << ff.fake_italic
                                << " has_opentype=" << ff.has_opentype
                                << " invalid_bbox=" << ff.invalid_bbox
                                << " cjk=" << ff.cjk
                                << " cjk_lang=" << ff.cjk_lang
                                << "}}";
                        return out.str();
                    }}
                    ''')

            # Direct access to fz_font_flags_t::ft_substitute for R2H-PDFpy,
            # while cppyy doesn't handle bitfields correctly.
            #
            cppyy.cppdef(f'''
                    int R2H-PDF_mfz_font_flags_ft_substitute( const fz_font_flags_t& ff)
                    {{
                        return ff.ft_substitute;
                    }}
                    ''')

            # Allow R2H-PDFpy to work - requires make_bookmark2() due to SWIG weirdness.
            #
            cppyy.gbl.R2H-PDF.make_bookmark2 = cppyy.gbl.R2H-PDF.make_bookmark
            cppyy.gbl.R2H-PDF.lookup_bookmark2 = cppyy.gbl.R2H-PDF.lookup_bookmark

            """)

    # Add auto-generate out-param wrappers - these modify fn wrappers to return
    # out-params as tuples.
    #
    #text += generated.cppyy_extra

    jlib.fs_ensure_parent_dir( path)
    jlib.fs_update( text, path)
