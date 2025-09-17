#ifdef GL_ES
  precision mediump float;
#endif

#if __VERSION__ >= 130
  #define COMPAT_OUT out
  #define COMPAT_IN in
  #define COMPAT_TEXTURE texture
#else
  #define COMPAT_OUT varying
  #define COMPAT_IN attribute
  #define COMPAT_TEXTURE texture2D
#endif

#ifdef VERTEX
  COMPAT_IN vec2 i_position;
  COMPAT_IN vec2 i_tex_coord;
  COMPAT_IN vec4 i_color;

  uniform vec2 u_view_size;

  COMPAT_OUT vec2 o_tex_coord;
  COMPAT_OUT vec4 o_color;

  vec2 view_to_world(vec2 view_size, vec2 world_position) {
    return vec2(
      2.0 * world_position.x / view_size.x - 1.0,
      -2.0 * world_position.y / view_size.y + 1.0
    );
  }

  void main() {
    o_tex_coord = i_tex_coord;
    // NOTE: div by 255.0 normalizes 0..255 u8 into 0.0..1.0 f32
    o_color = i_color / 255.0;
    gl_Position = vec4(view_to_world(u_view_size, i_position), 0.0, 1.0);
  }
#endif

#ifdef FRAGMENT
  COMPAT_OUT vec2 o_tex_coord;
  COMPAT_OUT vec4 o_color;

  uniform sampler2D u_sampler;

  void main() {
    gl_FragColor = o_color * COMPAT_TEXTURE(u_sampler, o_tex_coord);
  }
#endif
