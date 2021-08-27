{% import "macros.kt" as kt %}
{%- let obj = self.inner() %}
{%- let generic_t = "T" %}
public interface {{ obj.name()|class_name_kt }} {
    {% for meth in obj.methods() -%}
    fun <{{generic_t}}> {{ meth.name()|fn_name_kt }}(thunk: () -> {{generic_t}})
    {%- match meth.return_type() -%}
    {%- when Some with (return_type) %}: {#{{ return_type|type_kt -}}#}{{generic_t}}
    {%- else -%}
    {%- endmatch %}
    {% endfor %}
}
